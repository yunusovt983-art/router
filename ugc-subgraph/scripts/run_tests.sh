#!/bin/bash

# Test runner script for UGC Subgraph
# This script runs all types of tests in the correct order

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to run a command and check its exit status
run_command() {
    local cmd="$1"
    local description="$2"
    
    print_status "Running: $description"
    if eval "$cmd"; then
        print_success "$description completed successfully"
        return 0
    else
        print_error "$description failed"
        return 1
    fi
}

# Parse command line arguments
SKIP_UNIT=false
SKIP_INTEGRATION=false
SKIP_E2E=false
SKIP_PERFORMANCE=false
SKIP_BENCHMARKS=false
VERBOSE=false
COVERAGE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --skip-unit)
            SKIP_UNIT=true
            shift
            ;;
        --skip-integration)
            SKIP_INTEGRATION=true
            shift
            ;;
        --skip-e2e)
            SKIP_E2E=true
            shift
            ;;
        --skip-performance)
            SKIP_PERFORMANCE=true
            shift
            ;;
        --skip-benchmarks)
            SKIP_BENCHMARKS=true
            shift
            ;;
        --verbose)
            VERBOSE=true
            shift
            ;;
        --coverage)
            COVERAGE=true
            shift
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo "Options:"
            echo "  --skip-unit         Skip unit tests"
            echo "  --skip-integration  Skip integration tests"
            echo "  --skip-e2e          Skip end-to-end tests"
            echo "  --skip-performance  Skip performance tests"
            echo "  --skip-benchmarks   Skip benchmark tests"
            echo "  --verbose           Enable verbose output"
            echo "  --coverage          Generate code coverage report"
            echo "  --help              Show this help message"
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Set up environment
export RUST_LOG=debug
export RUST_BACKTRACE=1

if [ "$VERBOSE" = true ]; then
    export RUST_LOG=trace
fi

# Change to the ugc-subgraph directory
cd "$(dirname "$0")/.."

print_status "Starting test suite for UGC Subgraph"
print_status "Working directory: $(pwd)"

# Check if Docker is running (needed for integration tests)
if ! docker info > /dev/null 2>&1; then
    print_warning "Docker is not running. Integration and E2E tests will be skipped."
    SKIP_INTEGRATION=true
    SKIP_E2E=true
fi

# Build the project first
print_status "Building project..."
if ! cargo build; then
    print_error "Build failed"
    exit 1
fi
print_success "Build completed"

# Initialize test results
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Function to update test results
update_results() {
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    if [ $1 -eq 0 ]; then
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        FAILED_TESTS=$((FAILED_TESTS + 1))
    fi
}

# 1. Run unit tests
if [ "$SKIP_UNIT" = false ]; then
    print_status "Running unit tests..."
    
    if [ "$COVERAGE" = true ]; then
        # Install cargo-tarpaulin if not present
        if ! command -v cargo-tarpaulin &> /dev/null; then
            print_status "Installing cargo-tarpaulin for coverage..."
            cargo install cargo-tarpaulin
        fi
        
        run_command "cargo tarpaulin --lib --tests --out Html --output-dir target/coverage" "Unit tests with coverage"
        update_results $?
        
        if [ -f "target/coverage/tarpaulin-report.html" ]; then
            print_success "Coverage report generated: target/coverage/tarpaulin-report.html"
        fi
    else
        run_command "cargo test --lib" "Unit tests"
        update_results $?
    fi
else
    print_warning "Skipping unit tests"
fi

# 2. Run integration tests
if [ "$SKIP_INTEGRATION" = false ]; then
    print_status "Running integration tests..."
    
    # Run integration tests with timeout
    run_command "timeout 300 cargo test --test integration_tests --test auth_integration_tests" "Integration tests"
    update_results $?
else
    print_warning "Skipping integration tests"
fi

# 3. Run contract tests
print_status "Running contract tests..."
run_command "cargo test --test contract_tests --test schema_compatibility_tests" "Contract tests"
update_results $?

# 4. Run federation tests
print_status "Running federation tests..."
run_command "cargo test --test federation_tests" "Federation tests"
update_results $?

# 5. Run end-to-end tests
if [ "$SKIP_E2E" = false ]; then
    print_status "Running end-to-end tests..."
    
    # E2E tests might take longer
    run_command "timeout 600 cargo test --test e2e_tests" "End-to-end tests"
    update_results $?
else
    print_warning "Skipping end-to-end tests"
fi

# 6. Run performance tests
if [ "$SKIP_PERFORMANCE" = false ]; then
    print_status "Running performance tests..."
    
    # Performance tests can take a while
    run_command "timeout 900 cargo test --test performance_tests --release" "Performance tests"
    update_results $?
else
    print_warning "Skipping performance tests"
fi

# 7. Run benchmarks
if [ "$SKIP_BENCHMARKS" = false ]; then
    print_status "Running benchmarks..."
    
    # Install cargo-criterion if not present
    if ! command -v cargo-criterion &> /dev/null; then
        print_status "Installing cargo-criterion for benchmarks..."
        cargo install cargo-criterion
    fi
    
    run_command "cargo bench" "Benchmarks"
    update_results $?
    
    if [ -d "target/criterion" ]; then
        print_success "Benchmark reports generated in target/criterion/"
    fi
else
    print_warning "Skipping benchmarks"
fi

# 8. Run clippy for code quality
print_status "Running clippy for code quality checks..."
run_command "cargo clippy --all-targets --all-features -- -D warnings" "Clippy checks"
update_results $?

# 9. Run rustfmt for code formatting
print_status "Checking code formatting..."
run_command "cargo fmt -- --check" "Format checks"
update_results $?

# 10. Check for security vulnerabilities
print_status "Checking for security vulnerabilities..."
if ! command -v cargo-audit &> /dev/null; then
    print_status "Installing cargo-audit..."
    cargo install cargo-audit
fi
run_command "cargo audit" "Security audit"
update_results $?

# Print summary
echo ""
print_status "Test Summary"
echo "============"
echo "Total test suites: $TOTAL_TESTS"
echo "Passed: $PASSED_TESTS"
echo "Failed: $FAILED_TESTS"

if [ $FAILED_TESTS -eq 0 ]; then
    print_success "All tests passed! 🎉"
    
    # Generate final report
    echo ""
    print_status "Generating test report..."
    
    cat > target/test_report.md << EOF
# UGC Subgraph Test Report

Generated on: $(date)

## Test Results Summary

- **Total test suites**: $TOTAL_TESTS
- **Passed**: $PASSED_TESTS
- **Failed**: $FAILED_TESTS
- **Success rate**: $(( PASSED_TESTS * 100 / TOTAL_TESTS ))%

## Test Types Executed

- Unit tests: $([ "$SKIP_UNIT" = false ] && echo "✅" || echo "⏭️ Skipped")
- Integration tests: $([ "$SKIP_INTEGRATION" = false ] && echo "✅" || echo "⏭️ Skipped")
- Contract tests: ✅
- Federation tests: ✅
- End-to-end tests: $([ "$SKIP_E2E" = false ] && echo "✅" || echo "⏭️ Skipped")
- Performance tests: $([ "$SKIP_PERFORMANCE" = false ] && echo "✅" || echo "⏭️ Skipped")
- Benchmarks: $([ "$SKIP_BENCHMARKS" = false ] && echo "✅" || echo "⏭️ Skipped")
- Code quality (Clippy): ✅
- Code formatting: ✅
- Security audit: ✅

## Coverage Report

$([ "$COVERAGE" = true ] && echo "Coverage report available at: target/coverage/tarpaulin-report.html" || echo "Coverage not generated (use --coverage flag)")

## Benchmark Results

$([ "$SKIP_BENCHMARKS" = false ] && echo "Benchmark reports available at: target/criterion/" || echo "Benchmarks not run")

## Notes

- All tests were run in $([ "$VERBOSE" = true ] && echo "verbose" || echo "normal") mode
- Docker was $(docker info > /dev/null 2>&1 && echo "available" || echo "not available")
- Test environment: $(rustc --version)

EOF
    
    print_success "Test report generated: target/test_report.md"
    exit 0
else
    print_error "Some tests failed. Please check the output above."
    exit 1
fi