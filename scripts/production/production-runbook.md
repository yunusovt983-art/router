# Production Runbook - Apollo Router Federation

This runbook provides operational procedures for managing the Apollo Router Federation in production.

## Table of Contents

1. [System Overview](#system-overview)
2. [Emergency Procedures](#emergency-procedures)
3. [Common Issues](#common-issues)
4. [Monitoring and Alerting](#monitoring-and-alerting)
5. [Deployment Procedures](#deployment-procedures)
6. [Backup and Recovery](#backup-and-recovery)
7. [Performance Tuning](#performance-tuning)
8. [Security Procedures](#security-procedures)
9. [Contact Information](#contact-information)

## System Overview

### Architecture Components

- **Apollo Router**: Main GraphQL gateway (Port 4000)
- **UGC Subgraph**: User-generated content service (Port 4001)
- **Users Subgraph**: User management service (Port 4002)
- **Catalog Subgraph**: Vehicle catalog service (Port 4003)
- **Offers Subgraph**: Vehicle listings service (Port 4004)
- **Search Subgraph**: Search functionality service (Port 4005)

### Infrastructure

- **Kubernetes Cluster**: Production workloads
- **PostgreSQL**: Primary database
- **Redis**: Caching layer
- **Prometheus**: Metrics collection
- **Grafana**: Monitoring dashboards
- **Jaeger**: Distributed tracing

### Key URLs

- **Production API**: https://api.auto.ru/graphql
- **Grafana Dashboard**: https://grafana.auto.ru/d/apollo-federation
- **Prometheus**: https://prometheus.auto.ru
- **Jaeger**: https://jaeger.auto.ru

## Emergency Procedures

### 🚨 Complete System Outage

**Symptoms:**
- API returning 5xx errors
- All health checks failing
- No metrics being collected

**Immediate Actions:**
1. Check Kubernetes cluster status:
   ```bash
   kubectl get nodes
   kubectl get pods -n auto-ru-federation
   ```

2. Check Apollo Router status:
   ```bash
   kubectl logs -n auto-ru-federation deployment/apollo-router --tail=100
   kubectl describe pod -n auto-ru-federation -l app=apollo-router
   ```

3. If router is down, restart it:
   ```bash
   kubectl rollout restart deployment/apollo-router -n auto-ru-federation
   ```

4. Check subgraph health:
   ```bash
   kubectl get pods -n auto-ru-federation -l app!=apollo-router
   ```

5. If multiple services are down, check infrastructure:
   ```bash
   kubectl get pods -n auto-ru-federation
   kubectl get pvc -n auto-ru-federation
   kubectl top nodes
   ```

**Escalation:**
- If issue persists > 5 minutes, page on-call engineer
- If infrastructure issue, contact platform team

### 🔥 High Error Rate

**Symptoms:**
- Error rate > 5%
- Grafana alerts firing
- User complaints

**Immediate Actions:**
1. Check error patterns in Grafana dashboard
2. Review recent deployments:
   ```bash
   kubectl rollout history deployment/apollo-router -n auto-ru-federation
   ```

3. Check application logs:
   ```bash
   kubectl logs -n auto-ru-federation deployment/apollo-router --tail=200 | grep ERROR
   ```

4. If caused by recent deployment, rollback:
   ```bash
   kubectl rollout undo deployment/apollo-router -n auto-ru-federation
   ```

5. Check subgraph-specific errors:
   ```bash
   for service in ugc-subgraph users-subgraph offers-subgraph; do
     echo "=== $service ==="
     kubectl logs -n auto-ru-federation deployment/$service --tail=50 | grep ERROR
   done
   ```

### ⚡ High Response Time

**Symptoms:**
- P95 response time > 1 second
- Slow query alerts
- Performance degradation

**Immediate Actions:**
1. Check current load:
   ```bash
   kubectl top pods -n auto-ru-federation
   ```

2. Review slow queries in Grafana
3. Check database performance:
   ```bash
   kubectl exec -it postgresql-0 -n auto-ru-federation -- psql -U postgres -c "
   SELECT query, mean_time, calls 
   FROM pg_stat_statements 
   ORDER BY mean_time DESC 
   LIMIT 10;"
   ```

4. Check Redis cache hit rate
5. Scale up if needed:
   ```bash
   kubectl scale deployment apollo-router --replicas=5 -n auto-ru-federation
   ```

### 💾 Database Issues

**Symptoms:**
- Database connection errors
- Data inconsistency
- Slow database queries

**Immediate Actions:**
1. Check PostgreSQL status:
   ```bash
   kubectl get pods -n auto-ru-federation -l app.kubernetes.io/name=postgresql
   kubectl logs -n auto-ru-federation postgresql-0 --tail=100
   ```

2. Check database connections:
   ```bash
   kubectl exec -it postgresql-0 -n auto-ru-federation -- psql -U postgres -c "
   SELECT count(*) as active_connections 
   FROM pg_stat_activity 
   WHERE state = 'active';"
   ```

3. Check disk space:
   ```bash
   kubectl exec -it postgresql-0 -n auto-ru-federation -- df -h
   ```

4. If connection pool exhausted, restart affected services:
   ```bash
   kubectl rollout restart deployment/ugc-subgraph -n auto-ru-federation
   ```

## Common Issues

### Issue: GraphQL Introspection Exposed

**Symptoms:**
- Security scanner alerts
- Introspection queries succeeding in production

**Resolution:**
1. Check router configuration:
   ```bash
   kubectl get configmap apollo-router-config -n auto-ru-federation -o yaml
   ```

2. Ensure introspection is disabled:
   ```yaml
   supergraph:
     introspection: false
   ```

3. Update configuration and restart router:
   ```bash
   kubectl rollout restart deployment/apollo-router -n auto-ru-federation
   ```

### Issue: Rate Limiting Not Working

**Symptoms:**
- High request rates from single users
- No rate limit errors in logs

**Resolution:**
1. Check rate limiting configuration in subgraphs
2. Verify rate limiting middleware is enabled
3. Check Redis connectivity for rate limit storage:
   ```bash
   kubectl exec -it redis-master-0 -n auto-ru-federation -- redis-cli ping
   ```

### Issue: Cache Miss Rate High

**Symptoms:**
- Cache hit rate < 60%
- Increased database load
- Slower response times

**Resolution:**
1. Check Redis memory usage:
   ```bash
   kubectl exec -it redis-master-0 -n auto-ru-federation -- redis-cli info memory
   ```

2. Review cache TTL settings
3. Check for cache invalidation issues
4. Scale Redis if needed:
   ```bash
   kubectl scale statefulset redis-master --replicas=3 -n auto-ru-federation
   ```

### Issue: Subgraph Communication Failures

**Symptoms:**
- Federated queries failing
- Subgraph timeout errors
- Partial data returned

**Resolution:**
1. Check network connectivity between services:
   ```bash
   kubectl exec -it deployment/apollo-router -n auto-ru-federation -- \
     curl http://ugc-subgraph:4001/health
   ```

2. Check service discovery:
   ```bash
   kubectl get endpoints -n auto-ru-federation
   ```

3. Review subgraph logs for errors:
   ```bash
   kubectl logs -n auto-ru-federation deployment/ugc-subgraph --tail=100
   ```

## Monitoring and Alerting

### Key Metrics to Monitor

1. **Request Rate**: `rate(graphql_requests_total[5m])`
2. **Error Rate**: `rate(graphql_errors_total[5m]) / rate(graphql_requests_total[5m])`
3. **Response Time**: `histogram_quantile(0.95, rate(graphql_request_duration_seconds_bucket[5m]))`
4. **Cache Hit Rate**: `rate(cache_hits_total[5m]) / (rate(cache_hits_total[5m]) + rate(cache_misses_total[5m]))`
5. **Database Connections**: `database_connections_active`

### Alert Thresholds

| Metric | Warning | Critical |
|--------|---------|----------|
| Error Rate | > 2% | > 5% |
| P95 Response Time | > 500ms | > 1000ms |
| Cache Hit Rate | < 70% | < 50% |
| Database Connections | > 80% | > 95% |
| Memory Usage | > 80% | > 90% |

### Grafana Dashboards

1. **Apollo Federation Overview**: Main production dashboard
2. **Database Performance**: PostgreSQL metrics
3. **Cache Performance**: Redis metrics
4. **Security Monitoring**: Authentication and authorization metrics

### Alert Channels

- **Slack**: #alerts-production
- **PagerDuty**: Critical alerts
- **Email**: Non-critical alerts

## Deployment Procedures

### Standard Deployment

1. **Pre-deployment Checklist:**
   - [ ] All tests passing in CI/CD
   - [ ] Security scan completed
   - [ ] Performance tests passed
   - [ ] Database migrations reviewed
   - [ ] Rollback plan prepared

2. **Deployment Steps:**
   ```bash
   # Deploy to staging first
   ENVIRONMENT=staging ./scripts/production/deploy-production.sh
   
   # Run staging tests
   ./scripts/production/run-staging-tests.sh
   
   # Deploy to production
   ENVIRONMENT=production ./scripts/production/deploy-production.sh
   ```

3. **Post-deployment Verification:**
   - [ ] Health checks passing
   - [ ] Smoke tests successful
   - [ ] Metrics looking normal
   - [ ] No error rate increase

### Emergency Deployment

For critical hotfixes:

1. **Fast-track Process:**
   ```bash
   # Skip non-critical checks
   SKIP_BUILD=false \
   HEALTH_CHECK_TIMEOUT=60 \
   ./scripts/production/deploy-production.sh
   ```

2. **Immediate Monitoring:**
   - Watch error rates closely
   - Monitor response times
   - Check for any anomalies

### Rollback Procedures

1. **Automatic Rollback:**
   ```bash
   ./scripts/production/deploy-production.sh rollback
   ```

2. **Manual Rollback:**
   ```bash
   kubectl rollout undo deployment/apollo-router -n auto-ru-federation
   kubectl rollout undo deployment/ugc-subgraph -n auto-ru-federation
   # Repeat for other services
   ```

3. **Database Rollback:**
   - Review migration rollback scripts
   - Coordinate with DBA team
   - Test in staging first

## Backup and Recovery

### Database Backups

1. **Automated Backups:**
   - Daily full backups at 2 AM UTC
   - Hourly incremental backups
   - 30-day retention policy

2. **Manual Backup:**
   ```bash
   kubectl exec postgresql-0 -n auto-ru-federation -- \
     pg_dump -U postgres -d ugc_db > backup-$(date +%Y%m%d).sql
   ```

3. **Restore Procedure:**
   ```bash
   kubectl exec -i postgresql-0 -n auto-ru-federation -- \
     psql -U postgres -d ugc_db < backup-20240829.sql
   ```

### Configuration Backups

1. **Kubernetes Resources:**
   ```bash
   kubectl get all -n auto-ru-federation -o yaml > k8s-backup-$(date +%Y%m%d).yaml
   ```

2. **ConfigMaps and Secrets:**
   ```bash
   kubectl get configmaps,secrets -n auto-ru-federation -o yaml > config-backup-$(date +%Y%m%d).yaml
   ```

### Disaster Recovery

1. **RTO (Recovery Time Objective)**: 4 hours
2. **RPO (Recovery Point Objective)**: 1 hour

**Recovery Steps:**
1. Restore infrastructure (Kubernetes cluster)
2. Restore databases from backups
3. Deploy application services
4. Verify data integrity
5. Resume traffic

## Performance Tuning

### Database Optimization

1. **Connection Pool Tuning:**
   ```rust
   // Adjust based on load
   max_connections: 20,
   min_connections: 5,
   acquire_timeout: Duration::from_secs(30),
   ```

2. **Query Optimization:**
   - Review slow query logs
   - Add missing indexes
   - Optimize N+1 queries

3. **PostgreSQL Configuration:**
   ```sql
   -- Adjust based on available memory
   shared_buffers = '2GB'
   effective_cache_size = '6GB'
   work_mem = '256MB'
   ```

### Cache Optimization

1. **Redis Configuration:**
   ```
   maxmemory 4gb
   maxmemory-policy allkeys-lru
   ```

2. **Cache Strategy:**
   - Increase TTL for stable data
   - Implement cache warming
   - Use cache tags for invalidation

### Application Scaling

1. **Horizontal Scaling:**
   ```bash
   kubectl scale deployment apollo-router --replicas=5 -n auto-ru-federation
   ```

2. **Vertical Scaling:**
   ```yaml
   resources:
     requests:
       memory: 2Gi
       cpu: 1000m
     limits:
       memory: 4Gi
       cpu: 2000m
   ```

## Security Procedures

### Security Incident Response

1. **Immediate Actions:**
   - Isolate affected systems
   - Preserve evidence
   - Notify security team

2. **Investigation:**
   - Review access logs
   - Check for data breaches
   - Analyze attack vectors

3. **Remediation:**
   - Patch vulnerabilities
   - Update security controls
   - Monitor for recurrence

### Regular Security Tasks

1. **Weekly:**
   - Review security alerts
   - Check for failed login attempts
   - Verify SSL certificates

2. **Monthly:**
   - Run security scans
   - Review access permissions
   - Update security documentation

3. **Quarterly:**
   - Penetration testing
   - Security training
   - Incident response drills

### Security Monitoring

1. **Authentication Failures:**
   ```bash
   kubectl logs -n auto-ru-federation deployment/apollo-router | grep "auth.*failed"
   ```

2. **Suspicious Activities:**
   - Multiple failed login attempts
   - Unusual query patterns
   - High error rates from specific IPs

3. **Security Metrics:**
   - Failed authentication rate
   - Rate limiting violations
   - Query complexity violations

## Contact Information

### On-Call Rotation

- **Primary**: +7-XXX-XXX-XXXX (PagerDuty)
- **Secondary**: +7-XXX-XXX-XXXX (PagerDuty)

### Team Contacts

- **Platform Team**: platform@auto.ru
- **DevOps Team**: devops@auto.ru
- **Security Team**: security@auto.ru
- **Database Team**: dba@auto.ru

### Escalation Matrix

| Severity | Response Time | Escalation |
|----------|---------------|------------|
| Critical | 15 minutes | CTO, VP Engineering |
| High | 1 hour | Engineering Manager |
| Medium | 4 hours | Team Lead |
| Low | Next business day | Team Member |

### External Vendors

- **Cloud Provider**: support@cloudprovider.com
- **Monitoring**: support@grafana.com
- **Security**: support@security-vendor.com

---

**Document Version**: 1.0
**Last Updated**: $(date)
**Next Review**: $(date -d "+3 months")

This runbook should be reviewed and updated quarterly or after any major incidents.