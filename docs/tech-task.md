# ТЗ: CLI-утилита `obsidian-backupper`

## 1. Роль и контекст исполнителя
Вы — автономный Senior Rust Engineer. Ваша задача — разработать под ключ production-ready CLI-утилиту `obsidian-backupper` для macOS и Linux. Код должен быть строго типизирован, идиоматичен, покрыт модульными/интеграционными тестами и снабжён понятной обработкой ошибок.

---

## 2. Назначение и ключевой принцип
`obsidian-backupper` — консольная утилита для инкрементального шифрования заметок Obsidian Vault с помощью GnuPG и резервного копирования в удалённый приватный Git-репозиторий (GitHub).

* **Config-First:** Все рабочие параметры (пути, ключи GPG, репозиторий, фильтры) задаются в конфигурационном файле. Вызов команды `obsidian-backupper backup` должен выполняться без обязательных аргументов.
* **Идемпотентность и дифференциальный анализ:** Так как GnuPG использует случайную соль (рандомизированный вектор инициализации), повторное шифрование одного и того же файла порождает разные бинарные данные. Чтобы избежать ложных коммитов в Git, утилита отслеживает изменения по **SHA-256 хешу оригинального незашифрованного файла**.

---

## 3. Стек технологий и зависимости
* **Язык:** Rust (2024 edition или актуальный stable).
* **CLI интерфейс:** `clap` (с derive-макросами).
* **Конфигурация и сериализация:** `toml`, `serde`, `serde_json`.
* **Файловая система и пути:** `walkdir`, `dirs` (или `shellexpand`), `globset`.
* **Хеширование:** `sha2` (алгоритм SHA-256), `hex`.
* **Логирование и UI:** `tracing`, `tracing-subscriber`, `indicatif` (для прогресс-баров).
* **Обработка ошибок:** `thiserror` (для типизированных доменных ошибок), `anyhow` (для верхнеуровневого CLI-контекста).
* **Внешние вызовы (GPG, Git):** `std::process::Command` (для сохранения доступа к системным SSH-агентам и `gpg-agent`).

---

## 4. Спецификация конфигурации

### 4.1. Порядок резолвинга `config.toml`
1. Флаг CLI `--config <PATH>`.
2. Переменная окружения `OBSIDIAN_BACKUPPER_CONFIG`.
3. Системные каталоги:
   * **Linux:** `$XDG_CONFIG_HOME/obsidian-backupper/config.toml` (по умолчанию `~/.config/obsidian-backupper/config.toml`).
   * **macOS:** `~/Library/Application Support/obsidian-backupper/config.toml` или `~/.config/obsidian-backupper/config.toml`.
4. Локальный `.obsidian-backupper.toml` в текущей директории запуска.

### 4.2. Формат конфигурации (`config.toml`)
```toml
[paths]
source_dir = "~/Documents/MyObsidianVault"
target_dir = "~/Backups/ObsidianVaultEncrypted"

[encryption]
recipient = "user@example.com"
armor = false
gpg_binary = "gpg"

[git]
auto_push = true
remote_url = "git@github.com:username/obsidian-vault-encrypted.git"
branch = "main"
commit_message = "chore(backup): auto-sync {datetime}"

[filters]
exclude_patterns = [
  ".git/**",
  ".trash/**",
  ".obsidian/workspace.json",
  ".obsidian/cache/**",
  "**/.DS_Store",
  "**/thumbs.db"
]
include_obsidian_config = true