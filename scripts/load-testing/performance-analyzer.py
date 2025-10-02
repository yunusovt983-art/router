#!/usr/bin/env python3
"""
Performance Analyzer for Apollo Router Federation Load Tests

This script analyzes load test results and provides optimization recommendations.
"""

import json
import csv
import argparse
import os
import sys
from datetime import datetime
from typing import Dict, List, Any, Optional
import statistics
import matplotlib.pyplot as plt
import pandas as pd
import seaborn as sns

class PerformanceAnalyzer:
    def __init__(self, results_dir: str):
        self.results_dir = results_dir
        self.analysis_results = {}
        
    def analyze_k6_results(self, k6_file: str) -> Dict[str, Any]:
        """Analyze K6 test results"""
        try:
            with open(k6_file, 'r') as f:
                data = [json.loads(line) for line in f if line.strip()]
            
            # Extract metrics
            http_reqs = [d for d in data if d.get('type') == 'Point' and d.get('metric') == 'http_reqs']
            http_req_duration = [d for d in data if d.get('type') == 'Point' and d.get('metric') == 'http_req_duration']
            http_req_failed = [d for d in data if d.get('type') == 'Point' and d.get('metric') == 'http_req_failed']
            
            # Calculate statistics
            durations = [d['data']['value'] for d in http_req_duration]
            failed_requests = sum(d['data']['value'] for d in http_req_failed)
            total_requests = len(http_reqs)
            
            analysis = {
                'total_requests': total_requests,
                'failed_requests': failed_requests,
                'success_rate': (total_requests - failed_requests) / total_requests * 100 if total_requests > 0 else 0,
                'avg_response_time': statistics.mean(durations) if durations else 0,
                'p95_response_time': statistics.quantiles(durations, n=20)[18] if len(durations) > 20 else 0,
                'p99_response_time': statistics.quantiles(durations, n=100)[98] if len(durations) > 100 else 0,
                'min_response_time': min(durations) if durations else 0,
                'max_response_time': max(durations) if durations else 0,
                'throughput': total_requests / (max(d['data']['time'] for d in data) - min(d['data']['time'] for d in data)) if data else 0
            }
            
            return analysis
            
        except Exception as e:
            print(f"Error analyzing K6 results: {e}")
            return {}
    
    def analyze_artillery_results(self, artillery_file: str) -> Dict[str, Any]:
        """Analyze Artillery test results"""
        try:
            with open(artillery_file, 'r') as f:
                data = json.load(f)
            
            # Extract key metrics
            aggregate = data.get('aggregate', {})
            
            analysis = {
                'total_requests': aggregate.get('counters', {}).get('http.requests', 0),
                'total_responses': aggregate.get('counters', {}).get('http.responses', 0),
                'success_rate': (aggregate.get('counters', {}).get('http.codes.200', 0) / 
                               aggregate.get('counters', {}).get('http.responses', 1)) * 100,
                'avg_response_time': aggregate.get('latency', {}).get('mean', 0),
                'p95_response_time': aggregate.get('latency', {}).get('p95', 0),
                'p99_response_time': aggregate.get('latency', {}).get('p99', 0),
                'min_response_time': aggregate.get('latency', {}).get('min', 0),
                'max_response_time': aggregate.get('latency', {}).get('max', 0),
                'rps': aggregate.get('rps', {}).get('mean', 0),
                'errors': aggregate.get('counters', {}).get('errors.ECONNREFUSED', 0) + 
                         aggregate.get('counters', {}).get('errors.ETIMEDOUT', 0)
            }
            
            return analysis
            
        except Exception as e:
            print(f"Error analyzing Artillery results: {e}")
            return {}
    
    def analyze_resource_usage(self, resource_file: str) -> Dict[str, Any]:
        """Analyze system resource usage"""
        try:
            df = pd.read_csv(resource_file)
            
            # Clean and convert data
            df['CPU%'] = df['CPU%'].str.replace('%', '').astype(float)
            df['Memory%'] = df['Memory%'].str.replace('%', '').astype(float)
            
            analysis = {
                'avg_cpu_usage': df['CPU%'].mean(),
                'max_cpu_usage': df['CPU%'].max(),
                'avg_memory_usage': df['Memory%'].mean(),
                'max_memory_usage': df['Memory%'].max(),
                'cpu_spikes': len(df[df['CPU%'] > 80]),  # Count of high CPU usage
                'memory_spikes': len(df[df['Memory%'] > 80])  # Count of high memory usage
            }
            
            return analysis
            
        except Exception as e:
            print(f"Error analyzing resource usage: {e}")
            return {}
    
    def generate_performance_insights(self, analysis: Dict[str, Any]) -> List[str]:
        """Generate performance insights and recommendations"""
        insights = []
        
        # Response time analysis
        if analysis.get('p95_response_time', 0) > 1000:
            insights.append("⚠️  P95 response time exceeds 1 second - consider optimization")
        elif analysis.get('p95_response_time', 0) > 500:
            insights.append("⚡ P95 response time is acceptable but could be improved")
        else:
            insights.append("✅ Excellent response time performance")
        
        # Success rate analysis
        success_rate = analysis.get('success_rate', 0)
        if success_rate < 95:
            insights.append("🚨 Success rate below 95% - investigate error causes")
        elif success_rate < 99:
            insights.append("⚠️  Success rate could be improved")
        else:
            insights.append("✅ Excellent success rate")
        
        # Throughput analysis
        throughput = analysis.get('throughput', 0) or analysis.get('rps', 0)
        if throughput < 10:
            insights.append("📈 Low throughput - consider scaling or optimization")
        elif throughput < 50:
            insights.append("📊 Moderate throughput - room for improvement")
        else:
            insights.append("🚀 Good throughput performance")
        
        # Resource usage analysis
        if 'avg_cpu_usage' in analysis:
            cpu_usage = analysis['avg_cpu_usage']
            if cpu_usage > 80:
                insights.append("🔥 High CPU usage - consider horizontal scaling")
            elif cpu_usage > 60:
                insights.append("⚡ Moderate CPU usage - monitor under higher load")
            else:
                insights.append("✅ CPU usage is healthy")
        
        if 'avg_memory_usage' in analysis:
            memory_usage = analysis['avg_memory_usage']
            if memory_usage > 80:
                insights.append("💾 High memory usage - check for memory leaks")
            elif memory_usage > 60:
                insights.append("📊 Moderate memory usage - monitor growth")
            else:
                insights.append("✅ Memory usage is healthy")
        
        return insights
    
    def generate_optimization_recommendations(self, analysis: Dict[str, Any]) -> List[str]:
        """Generate specific optimization recommendations"""
        recommendations = []
        
        # Performance-based recommendations
        if analysis.get('p95_response_time', 0) > 500:
            recommendations.extend([
                "🔧 Implement Redis caching for frequently accessed data",
                "🔧 Optimize database queries and add appropriate indexes",
                "🔧 Consider implementing DataLoader pattern for N+1 query prevention",
                "🔧 Enable query result caching at the router level"
            ])
        
        if analysis.get('success_rate', 100) < 99:
            recommendations.extend([
                "🛡️  Implement circuit breakers for external service calls",
                "🛡️  Add retry logic with exponential backoff",
                "🛡️  Improve error handling and graceful degradation",
                "🛡️  Monitor and alert on error rate spikes"
            ])
        
        # Resource-based recommendations
        if analysis.get('avg_cpu_usage', 0) > 70:
            recommendations.extend([
                "⚡ Consider horizontal scaling (add more instances)",
                "⚡ Optimize CPU-intensive operations",
                "⚡ Implement connection pooling optimizations",
                "⚡ Profile application for CPU bottlenecks"
            ])
        
        if analysis.get('avg_memory_usage', 0) > 70:
            recommendations.extend([
                "💾 Investigate potential memory leaks",
                "💾 Optimize data structures and caching strategies",
                "💾 Implement memory-efficient pagination",
                "💾 Consider memory profiling tools"
            ])
        
        # Federation-specific recommendations
        recommendations.extend([
            "🌐 Monitor federated query performance separately",
            "🌐 Implement query complexity analysis",
            "🌐 Consider query whitelisting for production",
            "🌐 Optimize subgraph communication patterns"
        ])
        
        return recommendations
    
    def create_performance_charts(self, analysis: Dict[str, Any], output_dir: str):
        """Create performance visualization charts"""
        try:
            # Set up the plotting style
            plt.style.use('seaborn-v0_8')
            fig, axes = plt.subplots(2, 2, figsize=(15, 10))
            fig.suptitle('Apollo Router Federation Performance Analysis', fontsize=16)
            
            # Response time distribution
            if 'response_times' in analysis:
                axes[0, 0].hist(analysis['response_times'], bins=50, alpha=0.7, color='skyblue')
                axes[0, 0].set_title('Response Time Distribution')
                axes[0, 0].set_xlabel('Response Time (ms)')
                axes[0, 0].set_ylabel('Frequency')
            
            # Success rate over time
            if 'success_rate_timeline' in analysis:
                axes[0, 1].plot(analysis['success_rate_timeline'], color='green', linewidth=2)
                axes[0, 1].set_title('Success Rate Over Time')
                axes[0, 1].set_xlabel('Time')
                axes[0, 1].set_ylabel('Success Rate (%)')
                axes[0, 1].set_ylim(0, 100)
            
            # Resource usage
            if 'cpu_usage_timeline' in analysis and 'memory_usage_timeline' in analysis:
                axes[1, 0].plot(analysis['cpu_usage_timeline'], label='CPU %', color='red')
                axes[1, 0].plot(analysis['memory_usage_timeline'], label='Memory %', color='blue')
                axes[1, 0].set_title('Resource Usage Over Time')
                axes[1, 0].set_xlabel('Time')
                axes[1, 0].set_ylabel('Usage (%)')
                axes[1, 0].legend()
            
            # Throughput
            if 'throughput_timeline' in analysis:
                axes[1, 1].plot(analysis['throughput_timeline'], color='purple', linewidth=2)
                axes[1, 1].set_title('Throughput Over Time')
                axes[1, 1].set_xlabel('Time')
                axes[1, 1].set_ylabel('Requests/Second')
            
            plt.tight_layout()
            plt.savefig(os.path.join(output_dir, 'performance_charts.png'), dpi=300, bbox_inches='tight')
            plt.close()
            
            print(f"✅ Performance charts saved to {output_dir}/performance_charts.png")
            
        except Exception as e:
            print(f"Error creating charts: {e}")
    
    def generate_detailed_report(self, output_file: str):
        """Generate a detailed performance analysis report"""
        try:
            with open(output_file, 'w') as f:
                f.write("# Apollo Router Federation Performance Analysis Report\n\n")
                f.write(f"**Generated:** {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n\n")
                
                # Executive Summary
                f.write("## Executive Summary\n\n")
                f.write("This report provides a comprehensive analysis of the Apollo Router Federation ")
                f.write("performance under various load conditions.\n\n")
                
                # Key Metrics
                f.write("## Key Performance Metrics\n\n")
                for test_type, analysis in self.analysis_results.items():
                    f.write(f"### {test_type.title()} Test Results\n\n")
                    f.write(f"- **Total Requests:** {analysis.get('total_requests', 'N/A')}\n")
                    f.write(f"- **Success Rate:** {analysis.get('success_rate', 'N/A'):.2f}%\n")
                    f.write(f"- **Average Response Time:** {analysis.get('avg_response_time', 'N/A'):.2f}ms\n")
                    f.write(f"- **P95 Response Time:** {analysis.get('p95_response_time', 'N/A'):.2f}ms\n")
                    f.write(f"- **P99 Response Time:** {analysis.get('p99_response_time', 'N/A'):.2f}ms\n")
                    f.write(f"- **Throughput:** {analysis.get('throughput', analysis.get('rps', 'N/A')):.2f} req/s\n\n")
                
                # Performance Insights
                f.write("## Performance Insights\n\n")
                for test_type, analysis in self.analysis_results.items():
                    insights = self.generate_performance_insights(analysis)
                    if insights:
                        f.write(f"### {test_type.title()} Test Insights\n\n")
                        for insight in insights:
                            f.write(f"- {insight}\n")
                        f.write("\n")
                
                # Optimization Recommendations
                f.write("## Optimization Recommendations\n\n")
                all_recommendations = set()
                for analysis in self.analysis_results.values():
                    recommendations = self.generate_optimization_recommendations(analysis)
                    all_recommendations.update(recommendations)
                
                for i, recommendation in enumerate(sorted(all_recommendations), 1):
                    f.write(f"{i}. {recommendation}\n")
                
                f.write("\n")
                
                # Detailed Analysis
                f.write("## Detailed Analysis\n\n")
                f.write("### Response Time Analysis\n\n")
                f.write("Response time is a critical metric for user experience. ")
                f.write("The analysis shows the distribution of response times across different test scenarios.\n\n")
                
                f.write("### Throughput Analysis\n\n")
                f.write("Throughput measures the system's capacity to handle concurrent requests. ")
                f.write("Higher throughput indicates better scalability.\n\n")
                
                f.write("### Error Rate Analysis\n\n")
                f.write("Error rates indicate system reliability under load. ")
                f.write("Low error rates are essential for production readiness.\n\n")
                
                f.write("### Resource Utilization\n\n")
                f.write("Resource utilization helps identify bottlenecks and scaling needs. ")
                f.write("Monitor CPU and memory usage to plan capacity.\n\n")
                
                # Conclusion
                f.write("## Conclusion\n\n")
                f.write("The Apollo Router Federation demonstrates good performance characteristics ")
                f.write("under the tested load conditions. The recommendations above should be ")
                f.write("implemented to further optimize performance and ensure production readiness.\n\n")
                
                f.write("### Next Steps\n\n")
                f.write("1. Implement high-priority optimizations\n")
                f.write("2. Set up continuous performance monitoring\n")
                f.write("3. Establish performance baselines and SLAs\n")
                f.write("4. Plan for capacity scaling based on growth projections\n")
                f.write("5. Regular performance testing in CI/CD pipeline\n")
            
            print(f"✅ Detailed report generated: {output_file}")
            
        except Exception as e:
            print(f"Error generating report: {e}")
    
    def run_analysis(self):
        """Run the complete performance analysis"""
        print("🔍 Starting performance analysis...")
        
        # Find and analyze all result files
        for root, dirs, files in os.walk(self.results_dir):
            for file in files:
                file_path = os.path.join(root, file)
                
                if file.startswith('k6-') and file.endswith('-results.json'):
                    test_type = file.replace('k6-', '').replace('-results.json', '')
                    print(f"📊 Analyzing K6 {test_type} results...")
                    self.analysis_results[f'k6_{test_type}'] = self.analyze_k6_results(file_path)
                
                elif file == 'artillery-results.json':
                    print("📊 Analyzing Artillery results...")
                    self.analysis_results['artillery'] = self.analyze_artillery_results(file_path)
                
                elif file == 'resource-usage.log':
                    print("📊 Analyzing resource usage...")
                    self.analysis_results['resources'] = self.analyze_resource_usage(file_path)
        
        # Generate outputs
        output_dir = self.results_dir
        self.create_performance_charts(self.analysis_results, output_dir)
        self.generate_detailed_report(os.path.join(output_dir, 'performance_analysis_report.md'))
        
        print("✅ Performance analysis completed!")
        print(f"📁 Results available in: {output_dir}")

def main():
    parser = argparse.ArgumentParser(description='Analyze Apollo Router Federation load test results')
    parser.add_argument('results_dir', help='Directory containing test results')
    parser.add_argument('--output', '-o', help='Output directory for analysis results')
    
    args = parser.parse_args()
    
    if not os.path.exists(args.results_dir):
        print(f"❌ Results directory not found: {args.results_dir}")
        sys.exit(1)
    
    output_dir = args.output or args.results_dir
    
    analyzer = PerformanceAnalyzer(args.results_dir)
    analyzer.run_analysis()

if __name__ == '__main__':
    main()