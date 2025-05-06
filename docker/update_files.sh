#!/bin/bash

# Функция для копирования файла
copy_file() {
    local source=$1
    local dest=$2

    if [ ! -f "$source" ]; then
        echo "Ошибка: Исходный файл '$source' не существует."
        return 1
    fi

    cp -f "$source" "$dest"
    return $?
}

# Определение путей
ENV_SOURCE="../.env"
ENV_DEST="./"
BIN_SOURCE="../target/x86_64-unknown-linux-musl/release/svaha_mini_uploader_axum"
BIN_DEST="./app/"

# Копирование файлов
copy_file "$ENV_SOURCE" "$ENV_DEST" && \
copy_file "$BIN_SOURCE" "$BIN_DEST"

# Проверка результата
if [ $? -eq 0 ]; then
    echo "Файлы успешно скопированы и заменены."
else
    echo "Произошла ошибка при копировании файлов."
    exit 1
fi