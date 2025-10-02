#!/bin/bash

# =============================================================================
# Task 1: Настройка базовой инфраструктуры проекта
# Лог всех команд, выполненных AI для настройки инфраструктуры
# =============================================================================

echo "📋 Все команды, выполненные для Task 1: Настройка базовой инфраструктуры проекта"
echo "=================================================================="

# =============================================================================
# ЭТАП 1: ПРОВЕРКА КОМПИЛЯЦИИ WORKSPACE
# =============================================================================

echo ""
echo "🔧 ЭТАП 1: Проверка компиляции Rust workspace"
echo "----------------------------------------------"

# Команда: cargo check
# Назначение: Проверить компиляцию всего workspace без создания исполняемых файлов
# Результат: Обнаружена ошибка - отсутствуют main.rs файлы в подграфах
echo "cargo check  # Проверка компиляции workspace (первая попытка - неудачная)"

# =============================================================================
# ЭТАП 2: УСТАНОВКА DOCKER
# =============================================================================

echo ""
echo "🐳 ЭТАП 2: Установка Docker на Ubuntu 25.04"
echo "--------------------------------------------"

# Команда: which docker
# Назначение: Проверить, установлен ли Docker в системе
# Результат: Docker не найден (exit code 1)
echo "which docker  # Проверка наличия Docker в системе"

# Команда: cat /etc/os-release
# Назначение: Определить версию операционной системы для выбора правильного метода установки
# Результат: Ubuntu 25.04 (Plucky Puffin)
echo "cat /etc/os-release  # Определение версии ОС"

# Команда: sudo apt update
# Назначение: Обновить список пакетов из репозиториев
# Результат: Успешно обновлены индексы пакетов
echo "sudo apt update  # Обновление списка пакетов"

# Команда: sudo apt install -y apt-transport-https ca-certificates curl gnupg lsb-release
# Назначение: Установить необходимые зависимости для добавления Docker репозитория
# Результат: Пакеты уже установлены или успешно установлены
echo "sudo apt install -y apt-transport-https ca-certificates curl gnupg lsb-release  # Установка зависимостей"

# Команда: curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /usr/share/keyrings/docker-archive-keyring.gpg
# Назначение: Добавить официальный GPG ключ Docker для проверки подлинности пакетов
# Результат: GPG ключ успешно добавлен
echo "curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /usr/share/keyrings/docker-archive-keyring.gpg  # Добавление GPG ключа Docker"

# Команда: echo "deb [arch=amd64 signed-by=/usr/share/keyrings/docker-archive-keyring.gpg] https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null
# Назначение: Добавить официальный репозиторий Docker в список источников пакетов
# Результат: Репозиторий успешно добавлен
echo "echo \"deb [arch=amd64 signed-by=/usr/share/keyrings/docker-archive-keyring.gpg] https://download.docker.com/linux/ubuntu \$(lsb_release -cs) stable\" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null  # Добавление Docker репозитория"

# Команда: sudo apt update
# Назначение: Обновить список пакетов с учетом нового Docker репозитория
# Результат: Успешно получены пакеты из Docker репозитория
echo "sudo apt update  # Обновление с новым Docker репозиторием"

# Команда: sudo apt install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin
# Назначение: Установить Docker CE, CLI, containerd и Docker Compose plugin
# Результат: Успешно установлены все компоненты Docker (103 MB загружено)
echo "sudo apt install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin  # Установка Docker"

# Команда: sudo systemctl start docker
# Назначение: Запустить службу Docker
# Результат: Docker служба запущена
echo "sudo systemctl start docker  # Запуск службы Docker"

# Команда: sudo systemctl enable docker
# Назначение: Включить автозапуск Docker при загрузке системы
# Результат: Docker добавлен в автозапуск
echo "sudo systemctl enable docker  # Включение автозапуска Docker"

# Команда: sudo usermod -aG docker $USER
# Назначение: Добавить текущего пользователя в группу docker для работы без sudo
# Результат: Пользователь добавлен в группу (требует перезапуска сессии)
echo "sudo usermod -aG docker \$USER  # Добавление пользователя в группу docker"

# Команда: docker --version
# Назначение: Проверить версию установленного Docker
# Результат: Docker version 28.3.3, build 980b856
echo "docker --version  # Проверка версии Docker"

# Команда: docker compose version
# Назначение: Проверить версию Docker Compose plugin
# Результат: Docker Compose version v2.39.1
echo "docker compose version  # Проверка версии Docker Compose"

# Команда: sudo docker run hello-world
# Назначение: Тестовый запуск Docker для проверки работоспособности
# Результат: Успешно загружен и запущен hello-world контейнер
echo "sudo docker run hello-world  # Тестовый запуск Docker"

# =============================================================================
# ЭТАП 3: НАСТРОЙКА СКРИПТОВ И ЗАПУСК ИНФРАСТРУКТУРЫ
# =============================================================================

echo ""
echo "⚙️ ЭТАП 3: Настройка скриптов и запуск инфраструктуры"
echo "-----------------------------------------------------"

# Команда: chmod +x scripts/setup-dev.sh
# Назначение: Сделать скрипт настройки исполняемым
# Результат: Права на выполнение установлены
echo "chmod +x scripts/setup-dev.sh  # Установка прав на выполнение скрипта"

# Команда: ./scripts/setup-dev.sh (первая попытка)
# Назначение: Запустить скрипт настройки среды разработки
# Результат: Ошибка - скрипт не нашел docker-compose команду
echo "./scripts/setup-dev.sh  # Первый запуск скрипта (неудачный - нужно обновить проверку docker-compose)"

# После обновления скрипта для поддержки docker compose plugin:
# Команда: ./scripts/setup-dev.sh (вторая попытка)
# Назначение: Запустить обновленный скрипт настройки
# Результат: Успешно создана структура проекта, .env файл, проверена компиляция
echo "./scripts/setup-dev.sh  # Второй запуск скрипта (успешный)"

# Команда: make start-infra (с sudo)
# Назначение: Запустить инфраструктурные сервисы через Docker Compose
# Результат: Успешно запущены все 9 сервисов (PostgreSQL x4, Elasticsearch, Redis, Jaeger, Prometheus, Grafana)
echo "sudo make start-infra  # Запуск инфраструктурных сервисов"

# Команда: sudo docker compose ps
# Назначение: Проверить статус запущенных контейнеров
# Результат: Все 9 контейнеров работают корректно
echo "sudo docker compose ps  # Проверка статуса контейнеров"

# =============================================================================
# ЭТАП 4: ПРОВЕРКА РАБОТОСПОСОБНОСТИ СЕРВИСОВ
# =============================================================================

echo ""
echo "🔍 ЭТАП 4: Проверка работоспособности сервисов"
echo "----------------------------------------------"

# Команда: sudo docker compose logs elasticsearch | tail -10
# Назначение: Проверить логи Elasticsearch для диагностики
# Результат: Elasticsearch запущен и работает корректно
echo "sudo docker compose logs elasticsearch | tail -10  # Проверка логов Elasticsearch"

# Команда: sudo docker compose exec elasticsearch curl -s http://localhost:9200/_cluster/health
# Назначение: Проверить здоровье кластера Elasticsearch изнутри контейнера
# Результат: Кластер в состоянии "green" - полностью работоспособен
echo "sudo docker compose exec elasticsearch curl -s http://localhost:9200/_cluster/health  # Проверка здоровья Elasticsearch"

# Команда: ss -tlnp | grep :9200
# Назначение: Проверить, что порт 9200 прослушивается на хосте
# Результат: Порт открыт и доступен
echo "ss -tlnp | grep :9200  # Проверка открытых портов"

# Команда: chmod +x scripts/health-check.sh
# Назначение: Сделать скрипт проверки здоровья исполняемым
# Результат: Права установлены
echo "chmod +x scripts/health-check.sh  # Установка прав на скрипт проверки здоровья"

# Команда: ./scripts/health-check.sh
# Назначение: Запустить комплексную проверку всех сервисов
# Результат: Все 9 сервисов работают корректно и доступны
echo "./scripts/health-check.sh  # Комплексная проверка здоровья всех сервисов"

# Команда: make health-check
# Назначение: Запустить проверку здоровья через Makefile
# Результат: Все сервисы работают корректно
echo "make health-check  # Проверка здоровья через Makefile"

# =============================================================================
# ЭТАП 5: ФИНАЛЬНАЯ ПРОВЕРКА КОМПИЛЯЦИИ
# =============================================================================

echo ""
echo "✅ ЭТАП 5: Финальная проверка компиляции"
echo "----------------------------------------"

# Команда: cargo check (финальная проверка)
# Назначение: Убедиться, что весь workspace компилируется после создания всех файлов
# Результат: Успешная компиляция всех 5 подграфов
echo "cargo check  # Финальная проверка компиляции workspace (успешная)"

# =============================================================================
# ИТОГОВАЯ СТАТИСТИКА
# =============================================================================

echo ""
echo "📊 ИТОГОВАЯ СТАТИСТИКА Task 1"
echo "============================="
echo "✅ Создано файлов: ~30+ (Cargo.toml, Dockerfile, конфигурации, скрипты)"
echo "✅ Установлен Docker: версия 28.3.3 с Compose plugin v2.39.1"
echo "✅ Запущено сервисов: 9 (PostgreSQL x4, Elasticsearch, Redis, Jaeger, Prometheus, Grafana)"
echo "✅ Настроено портов: 10+ (4000, 4001-4005, 5432-5435, 6379, 9200, 9091, 3000, 16686, 14268)"
echo "✅ Workspace компилируется: 5 подграфов готовы к разработке"
echo "✅ Инфраструктура готова: полная среда разработки для GraphQL федерации"

echo ""
echo "🎯 РЕЗУЛЬТАТ: Task 1 выполнен на 100%"
echo "Инфраструктура Auto.ru GraphQL Federation полностью настроена и готова к разработке!"

# =============================================================================
# ПОЛЕЗНЫЕ КОМАНДЫ ДЛЯ ДАЛЬНЕЙШЕЙ РАБОТЫ
# =============================================================================

echo ""
echo "🚀 ПОЛЕЗНЫЕ КОМАНДЫ ДЛЯ ДАЛЬНЕЙШЕЙ РАБОТЫ:"
echo "=========================================="
echo "make health-check     # Проверка здоровья всех сервисов"
echo "make start-infra      # Запуск инфраструктуры"
echo "make stop             # Остановка всех сервисов"
echo "make logs-ugc         # Просмотр логов UGC подграфа"
echo "make dev-ugc          # Запуск UGC подграфа в режиме разработки"
echo "cargo build           # Сборка всех подграфов"
echo "cargo test            # Запуск тестов"
echo ""
echo "🌐 ДОСТУПНЫЕ URL:"
echo "http://localhost:4000  # Apollo Router (когда будет запущен)"
echo "http://localhost:3000  # Grafana (admin/admin)"
echo "http://localhost:16686 # Jaeger UI"
echo "http://localhost:9091  # Prometheus"
echo "http://localhost:9200  # Elasticsearch"