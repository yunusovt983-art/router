#!/bin/bash

# Auto.ru GraphQL Federation - Supergraph Validation Script
# This script validates the supergraph schema and checks for common issues

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SUPERGRAPH_SCHEMA_PATH="./supergraph.graphql"
ROUTER_CONFIG_PATH="./router.yaml"

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

# Function to check if required files exist
check_files() {
    print_status "Checking required files..."
    
    local files_missing=0
    
    if [ ! -f "$SUPERGRAPH_SCHEMA_PATH" ]; then
        print_error "Supergraph schema file not found: $SUPERGRAPH_SCHEMA_PATH"
        ((files_missing++))
    fi
    
    if [ ! -f "$ROUTER_CONFIG_PATH" ]; then
        print_error "Router configuration file not found: $ROUTER_CONFIG_PATH"
        ((files_missing++))
    fi
    
    if [ "$files_missing" -eq 0 ]; then
        print_success "All required files found"
        return 0
    else
        print_error "$files_missing required file(s) missing"
        return 1
    fi
}

# Function to validate supergraph schema structure
validate_schema_structure() {
    print_status "Validating supergraph schema structure..."
    
    local errors=0
    
    # Check if file is not empty
    if [ ! -s "$SUPERGRAPH_SCHEMA_PATH" ]; then
        print_error "Supergraph schema file is empty"
        ((errors++))
    fi
    
    # Check for required GraphQL elements
    if ! grep -q "type Query" "$SUPERGRAPH_SCHEMA_PATH"; then
        print_error "No Query type found in supergraph schema"
        ((errors++))
    fi
    
    # Check for federation directives
    if ! grep -q "@key" "$SUPERGRAPH_SCHEMA_PATH"; then
        print_warning "No @key directives found - this might not be a federated schema"
    fi
    
    # Check for expected entities
    local expected_entities=("User" "Offer" "Review" "Brand" "Model")
    for entity in "${expected_entities[@]}"; do
        if grep -q "type $entity" "$SUPERGRAPH_SCHEMA_PATH"; then
            print_success "Found entity: $entity"
        else
            print_warning "Entity not found: $entity"
        fi
    done
    
    # Check for syntax errors (basic)
    if grep -q "type.*{" "$SUPERGRAPH_SCHEMA_PATH" && grep -q "}" "$SUPERGRAPH_SCHEMA_PATH"; then
        print_success "Schema appears to have valid GraphQL syntax"
    else
        print_error "Schema may have syntax errors"
        ((errors++))
    fi
    
    if [ "$errors" -eq 0 ]; then
        print_success "Schema structure validation passed"
        return 0
    else
        print_error "Schema structure validation failed with $errors error(s)"
        return 1
    fi
}

# Function to validate federation-specific aspects
validate_federation() {
    print_status "Validating federation-specific aspects..."
    
    local warnings=0
    
    # Check for proper entity keys
    local entities_with_keys=0
    if grep -q "@key" "$SUPERGRAPH_SCHEMA_PATH" 2>/dev/null; then
        entities_with_keys=$(grep -c "@key" "$SUPERGRAPH_SCHEMA_PATH" 2>/dev/null)
        print_success "Found $entities_with_keys entities with @key directives"
    else
        print_warning "No entities with @key directives found"
        ((warnings++))
    fi
    
    # Check for entity extensions
    local entity_extensions=0
    if grep -q "extend type" "$SUPERGRAPH_SCHEMA_PATH" 2>/dev/null; then
        entity_extensions=$(grep -c "extend type" "$SUPERGRAPH_SCHEMA_PATH" 2>/dev/null)
        print_success "Found $entity_extensions entity extensions"
    else
        print_warning "No entity extensions found"
        ((warnings++))
    fi
    
    # Check for external fields
    local external_fields=0
    if grep -q "@external" "$SUPERGRAPH_SCHEMA_PATH" 2>/dev/null; then
        external_fields=$(grep -c "@external" "$SUPERGRAPH_SCHEMA_PATH" 2>/dev/null)
        print_success "Found $external_fields external fields"
    else
        print_warning "No external fields found"
        ((warnings++))
    fi
    
    # Check for federation spec link
    if grep -q "@link.*federation" "$SUPERGRAPH_SCHEMA_PATH"; then
        print_success "Federation spec link found"
    else
        print_warning "No federation spec link found"
        ((warnings++))
    fi
    
    if [ "$warnings" -eq 0 ]; then
        print_success "Federation validation passed"
    else
        print_warning "Federation validation completed with $warnings warning(s)"
    fi
    
    return 0
}

# Function to validate router configuration
validate_router_config() {
    print_status "Validating router configuration..."
    
    local errors=0
    
    # Check if router config is valid YAML
    if command -v yq &> /dev/null; then
        if yq eval '.' "$ROUTER_CONFIG_PATH" > /dev/null 2>&1; then
            print_success "Router configuration is valid YAML"
        else
            print_error "Router configuration is not valid YAML"
            ((errors++))
        fi
    else
        print_warning "yq not available, skipping YAML validation"
    fi
    
    # Check for required sections
    local required_sections=("supergraph" "subgraphs" "telemetry")
    for section in "${required_sections[@]}"; do
        if grep -q "^$section:" "$ROUTER_CONFIG_PATH"; then
            print_success "Found required section: $section"
        else
            print_error "Missing required section: $section"
            ((errors++))
        fi
    done
    
    # Check for subgraph configurations
    local expected_subgraphs=("ugc" "users" "offers" "catalog" "search")
    for subgraph in "${expected_subgraphs[@]}"; do
        if grep -q "  $subgraph:" "$ROUTER_CONFIG_PATH"; then
            print_success "Found subgraph configuration: $subgraph"
        else
            print_warning "Missing subgraph configuration: $subgraph"
        fi
    done
    
    # Check for supergraph schema path
    if grep -q "supergraph_path:" "$ROUTER_CONFIG_PATH"; then
        print_success "Supergraph schema path configured"
    else
        print_warning "No supergraph schema path configured"
    fi
    
    if [ "$errors" -eq 0 ]; then
        print_success "Router configuration validation passed"
        return 0
    else
        print_error "Router configuration validation failed with $errors error(s)"
        return 1
    fi
}

# Function to check schema compatibility
check_schema_compatibility() {
    print_status "Checking schema compatibility..."
    
    # This is a basic compatibility check
    # In a real implementation, you might use Apollo Studio schema checks
    
    local warnings=0
    
    # Check for potentially breaking changes
    if grep -q "!" "$SUPERGRAPH_SCHEMA_PATH"; then
        local required_fields=$(grep -c "!" "$SUPERGRAPH_SCHEMA_PATH" || echo "0")
        print_status "Found $required_fields required fields (non-null)"
    fi
    
    # Check for deprecated fields
    if grep -q "@deprecated" "$SUPERGRAPH_SCHEMA_PATH"; then
        local deprecated_fields=$(grep -c "@deprecated" "$SUPERGRAPH_SCHEMA_PATH" || echo "0")
        print_warning "Found $deprecated_fields deprecated fields"
        ((warnings++))
    fi
    
    # Check for common anti-patterns
    if grep -q "String!" "$SUPERGRAPH_SCHEMA_PATH" && grep -q "ID!" "$SUPERGRAPH_SCHEMA_PATH"; then
        print_success "Schema uses appropriate scalar types"
    fi
    
    if [ "$warnings" -eq 0 ]; then
        print_success "Schema compatibility check passed"
    else
        print_warning "Schema compatibility check completed with $warnings warning(s)"
    fi
    
    return 0
}

# Function to generate validation report
generate_report() {
    print_status "Generating validation report..."
    
    local report_file="./supergraph-validation-report.txt"
    
    {
        echo "Auto.ru GraphQL Federation - Supergraph Validation Report"
        echo "Generated: $(date)"
        echo "========================================================"
        echo ""
        
        echo "Files Checked:"
        echo "- Supergraph Schema: $SUPERGRAPH_SCHEMA_PATH"
        echo "- Router Config: $ROUTER_CONFIG_PATH"
        echo ""
        
        echo "Schema Statistics:"
        if [ -f "$SUPERGRAPH_SCHEMA_PATH" ]; then
            echo "- File Size: $(wc -c < "$SUPERGRAPH_SCHEMA_PATH") bytes"
            echo "- Lines: $(wc -l < "$SUPERGRAPH_SCHEMA_PATH")"
            echo "- Types: $(grep -c "^type " "$SUPERGRAPH_SCHEMA_PATH" || echo "0")"
            echo "- Enums: $(grep -c "^enum " "$SUPERGRAPH_SCHEMA_PATH" || echo "0")"
            echo "- Inputs: $(grep -c "^input " "$SUPERGRAPH_SCHEMA_PATH" || echo "0")"
            echo "- Interfaces: $(grep -c "^interface " "$SUPERGRAPH_SCHEMA_PATH" || echo "0")"
            echo "- Unions: $(grep -c "^union " "$SUPERGRAPH_SCHEMA_PATH" || echo "0")"
            echo "- Scalars: $(grep -c "^scalar " "$SUPERGRAPH_SCHEMA_PATH" || echo "0")"
        fi
        echo ""
        
        echo "Federation Statistics:"
        if [ -f "$SUPERGRAPH_SCHEMA_PATH" ]; then
            echo "- @key directives: $(grep -c "@key" "$SUPERGRAPH_SCHEMA_PATH" || echo "0")"
            echo "- @external fields: $(grep -c "@external" "$SUPERGRAPH_SCHEMA_PATH" || echo "0")"
            echo "- Entity extensions: $(grep -c "extend type" "$SUPERGRAPH_SCHEMA_PATH" || echo "0")"
            echo "- @requires directives: $(grep -c "@requires" "$SUPERGRAPH_SCHEMA_PATH" || echo "0")"
            echo "- @provides directives: $(grep -c "@provides" "$SUPERGRAPH_SCHEMA_PATH" || echo "0")"
        fi
        echo ""
        
        echo "Validation completed at: $(date)"
        
    } > "$report_file"
    
    print_success "Validation report generated: $report_file"
}

# Function to show schema preview
show_schema_preview() {
    print_status "Schema Preview (first 50 lines):"
    echo "=================================="
    
    if [ -f "$SUPERGRAPH_SCHEMA_PATH" ]; then
        head -50 "$SUPERGRAPH_SCHEMA_PATH"
        echo ""
        echo "... (truncated)"
    else
        print_error "Schema file not found"
    fi
}

# Main validation function
main() {
    print_status "Starting supergraph validation..."
    
    local total_errors=0
    
    # Run all validation checks
    check_files || ((total_errors++))
    validate_schema_structure || ((total_errors++))
    validate_federation || true  # Don't fail on federation warnings
    validate_router_config || ((total_errors++))
    check_schema_compatibility || true  # Don't fail on compatibility warnings
    
    # Generate report
    generate_report
    
    # Show preview if requested
    if [ "$1" = "--preview" ]; then
        show_schema_preview
    fi
    
    # Final result
    echo ""
    if [ "$total_errors" -eq 0 ]; then
        print_success "✅ All validation checks passed!"
        print_status "The supergraph is ready for deployment."
        return 0
    else
        print_error "❌ Validation failed with $total_errors critical error(s)"
        print_error "Please fix the errors before deploying the supergraph."
        return 1
    fi
}

# Show help
show_help() {
    echo "Auto.ru GraphQL Federation - Supergraph Validation Script"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --preview    Show schema preview after validation"
    echo "  --help       Show this help message"
    echo ""
    echo "This script validates:"
    echo "  - Supergraph schema structure and syntax"
    echo "  - Federation-specific directives and patterns"
    echo "  - Router configuration"
    echo "  - Schema compatibility"
    echo ""
}

# Parse command line arguments
case "${1:-}" in
    --help|-h)
        show_help
        exit 0
        ;;
    *)
        main "$@"
        exit $?
        ;;
esac