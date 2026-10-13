# ТЗ: CLI-утилита `obsidian-backupper`

## 1. Роль и контекст исполнителя

Вы — автономный Senior Rust Engineer. Ваша задача — разработать под ключ production-ready CLI-утилиту `obsidian-backupper` для macOS и Linux. Код должен быть строго типизирован, идиоматичен, покрыт модульными/интеграционными тестами и снабжён понятной обработкой ошибок.

## 2. Назначение и ключевой принцип

`obsidian-backupper` — консольная утилита для инкрементального шифрования заметок Obsidian Vault с помощью GnuPG и резервного копирования в удалённый приватный Git-репозиторий (GitHub).

- **Config-First:** Все рабочие параметры (пути, ключи GPG, репозиторий, фильтры) задаются в конфигурационном файле. Вызов команды `obsidian-backupper backup` должен выполняться без обязательных аргументов.
    
- **Идемпотентность и дифференциальный анализ:** Так как GnuPG использует случайную соль (рандомизированный вектор инициализации), повторное шифрование одного и того же файла порождает разные бинарные данные. Чтобы избежать ложных коммитов в Git, утилита отслеживает изменения по **SHA-256 хешу оригинального незашифрованного файла**.
    

## 3. Стек технологий и зависимости

- **Язык:** Rust (2024 edition или актуальный stable).
    
- **CLI интерфейс:** `clap` (с derive-макросами).
    
- **Конфигурация и сериализация:** `toml`, `serde`, `serde_json`.
    
- **Файловая система и пути:** `walkdir`, `dirs` (или `shellexpand`), `globset`.
    
- **Хеширование:** `sha2` (алгоритм SHA-256), `hex`.
    
- **Логирование и UI:** `tracing`, `tracing-subscriber`, `indicatif` (для прогресс-баров).
    
- **Обработка ошибок:** `thiserror` (для типизированных доменных ошибок), `anyhow` (для верхнеуровневого CLI-контекста).
    
- **Внешние вызовы (GPG, Git):** `std::process::Command` (для сохранения доступа к системным SSH-агентам и `gpg-agent`).
    

## 4. Спецификация конфигурации

### 4.1. Порядок резолвинга `config.toml`

1. Флаг CLI `--config <PATH>`.
    
2. Переменная окружения `OBSIDIAN_BACKUPPER_CONFIG`.
    
3. Системные каталоги:
    
    - **Linux:** `$XDG_CONFIG_HOME/obsidian-backupper/config.toml` (по умолчанию `~/.config/obsidian-backupper/config.toml`).
        
    - **macOS:** `~/Library/Application Support/obsidian-backupper/config.toml` или `~/.config/obsidian-backupper/config.toml`.
        
4. Локальный `.obsidian-backupper.toml` в текущей директории запуска.
    

### 4.2. Формат конфигурации (`config.toml`)

Ini, TOML

```
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
```

## 5. Структура директорий и файл манифеста

### 5.1. Каталог назначения (`target_dir`)

Plaintext

```
target_dir/
├── .git/
├── .sync_manifest.json
└── vault/
    ├── Notes/
    │   └── Daily.md.gpg
    └── Attachments/
        └── diagram.png.gpg
```

### 5.2. Схема `.sync_manifest.json`

Манифест хранится в корне `target_dir` и отслеживается в Git:

JSON

```
{
  "version": 1,
  "last_sync_timestamp": 1774000000,
  "entries": {
    "Notes/Daily.md": {
      "plain_sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "target_rel_path": "vault/Notes/Daily.md.gpg",
      "modified_at_epoch": 1773999900,
      "size_bytes": 1024
    }
  }
}
```

## 6. Алгоритм дифференциального анализа и бэкапа

1. **Инициализация окружения:**
    
    - Раскрыть тильды (`~`) во всех путях.
        
    - Проверить существование `source_dir`.
        
    - Создать `target_dir`, если отсутствует.
        
    - Если в `target_dir` нет `.git`: выполнить `git init`, `git remote add origin <remote_url>`, создать базовый `.gitignore` (исключить `*.tmp`).
        
    - Проверить наличие ключа `recipient`: `gpg --list-keys <recipient>`.
        
2. **Сканирование `source_dir`:**
    
    - Обойти все файлы рекурсивно (`walkdir`).
        
    - Отфильтровать файлы согласно `exclude_patterns`. Привести пути к относительному POSIX-формату (разделитель `/`).
        
    - Вычислить SHA-256 хеш содержимого каждого файла.
        
3. **Чтение манифеста:**
    
    - Загрузить `.sync_manifest.json` из `target_dir`. Если файл отсутствует — считать манифест пустым.
        
4. **Сравнение состояний (Diff Engine):**
    
    - **CREATE:** Файл есть в `source_dir`, но отсутствует в манифесте.
        
    - **UPDATE:** Файл есть в обоих списках, но `current_sha256 != manifest.plain_sha256`.
        
    - **DELETE:** Запись есть в манифесте, но файл отсутствует в `source_dir`.
        
    - **SKIP:** Запись есть в обоих списках и `current_sha256 == manifest.plain_sha256`.
        
5. **Применение изменений:**
    
    - **Для CREATE / UPDATE:**
        
        - Создать родительские папки в `target_dir/vault/...`.
            
        - Выполнить шифрование во временный файл:
            
            Bash
            
            ```
            gpg --batch --yes --encrypt -r <recipient> -o <target_dir/vault/.../file.md.gpg.tmp> <source_file>
            ```
            
        - Атомарно переименовать `.tmp` в `.gpg` (`std::fs::rename`).
            
        - Обновить запись в манифесте.
            
    - **Для DELETE:**
        
        - Удалить файл `<target_dir/vault/.../file.md.gpg>`.
            
        - Удалить запись из манифеста.
            
        - Рекурсивно удалить пустые поддиректории в `target_dir/vault/`.
            
6. **Сохранение и Git-синхронизация:**
    
    - Атомарно перезаписать `.sync_manifest.json` (через `.tmp` и `rename`).
        
    - Если `git.auto_push == true`:
        
        - Выполнить `git add -A`.
            
        - Проверить статус через `git status --porcelain`. Если изменений нет — завершить работу без коммита.
            
        - Если изменения есть: `git commit -m "<commit_message>"` (подставив текущее UTC-время вместо `{datetime}`) и `git push origin <branch>`.
            

## 7. Команды интерфейса (CLI)

Bash

```
# Инициализация конфигурации
obsidian-backupper init

# Выполнение бэкапа
obsidian-backupper backup [--dry-run] [--verbose]

# Восстановление (дешифрование) файлов из бэкапа
obsidian-backupper restore [--target <DIR>]

# Проверка окружения (доступность gpg ключа, git remote, директорий)
obsidian-backupper check
```

- Флаг `--dry-run` выполняет сканирование и вывод таблицы запланированных действий (CREATE, UPDATE, DELETE, SKIP) без изменения файлов на диске и без вызова Git.
    

## 8. Критерии приёмки (Definition of Done)

1. Код компилируется без ворнингов: `cargo check` и `cargo clippy -- -D warnings`.
    
2. Код отформатирован: `cargo fmt -- --check`.
    
3. Все операции с файлами атомарны (используются временные `.tmp` файлы).
    
4. Написаны unit-тесты:
    
    - Тест парсинга `config.toml` и раскрытия тильды `~`.
        
    - Тест логики сравнения диффов (`diff.rs`) с моковыми входными данными.
        
    - Тест фильтрации путей через `globset`.
        
5. Написан интеграционный тест (`tests/backup_test.rs`), проверяющий полный цикл backup/restore во временных каталогах `tempfile`.