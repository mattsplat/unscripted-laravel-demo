#!/bin/sh
# First start: the SQLite database and an app key, made on the volume at storage/app/data (which
# keeps them after). The key's in the .env there (.env links to it), unless APP_KEY's set.
DATA=/var/www/html/storage/app/data
mkdir -p "$DATA"
[ -f "$DB_DATABASE" ] || touch "$DB_DATABASE"
if [ -z "$APP_KEY" ] && ! grep -qs '^APP_KEY=base64:' "$DATA/.env"; then
    echo "APP_KEY=base64:$(head -c 32 /dev/urandom | base64)" >> "$DATA/.env"
fi
