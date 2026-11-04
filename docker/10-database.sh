#!/bin/sh
# The SQLite database, made on first start (the volume at storage/app/data keeps it after).
mkdir -p "$(dirname "$DB_DATABASE")"
[ -f "$DB_DATABASE" ] || touch "$DB_DATABASE"
