# Task 9: Deployment Diagram - Подробное объяснение production инфраструктуры оптимизации производительности

## 🎯 Цель диаграммы

Deployment диаграмма Task 9 демонстрирует **production-ready инфраструктуру оптимизации производительности** для GraphQL федерации Auto.ru в AWS облаке, показывая как компоненты кеширования, DataLoader оптимизации и rate limiting развертываются, масштабируются и интегрируются с управляемыми сервисами AWS для обеспечения enterprise-grade производительности.

## 🏗️ Архитектурная эволюция: от development к production

### От локальной разработки к облачной инфраструктуре оптимизации

#### Было: Локальная разработка без оптимизации
```yaml
# docker-compose.dev.yml - Development setup без оптимизации
version: '3.8'
services:
  # Простая локальная настройка
  ugc-service:
    build: ./ugc-subgraph
    ports:
      - "4001:4001"
    environment:
      - DATABASE_URL=postgresql://user:pass@postgres:5432/ugc
    depends_on:
      - postgres
    # Проблемы:
    # - Нет кеширования
    # - Нет connection pooling
    # - Нет rate limiting
    # - Нет performance monitoring
    # - Нет масштабирования

  postgres:
    image: postgres:15
    environment:
      - POSTGRES_DB=ugc
      - POSTGRES_USER=user
      - POSTGRES_PASSWORD=pass
    # Проблемы:
    # - Нет оптимизации производительности
    # - Нет read replicas
    # - Нет connection pooling
    # - Нет мониторинга медленных запросов
```