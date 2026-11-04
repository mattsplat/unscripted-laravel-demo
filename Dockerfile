# The whole app (pages, components, the games' WebAssembly), for a host that runs containers
# (Coolify). Outpost's game server is its own image: outpost/server/Dockerfile.
#
#   docker build -t unscripted-demo .
#   docker run -p 8080:8080 -e APP_KEY=base64:... -v demo-data:/var/www/html/storage/app/data unscripted-demo
#
# nginx and PHP-FPM (serversideup/php), on 8080. The database is SQLite on the volume at
# storage/app/data; migrations run on every start. Calls go over HTTP (no Reverb), so #[Live]
# boards poll.
FROM serversideup/php:8.4-fpm-nginx

ENV PHP_OPCACHE_ENABLE=1 \
    AUTORUN_ENABLED=true \
    AUTORUN_LARAVEL_STORAGE_LINK=false \
    APP_ENV=production \
    APP_DEBUG=false \
    LOG_CHANNEL=stderr \
    DB_CONNECTION=sqlite \
    DB_DATABASE=/var/www/html/storage/app/data/database.sqlite \
    UNSCRIPTED_TRANSPORT=http \
    BROADCAST_CONNECTION=log

COPY --chown=www-data:www-data composer.json composer.lock ./
RUN composer install --no-dev --no-interaction --no-autoloader --no-scripts --prefer-dist

COPY --chown=www-data:www-data . .
RUN composer dump-autoload --optimize --no-dev \
    && mkdir -p storage/app/data storage/framework/cache storage/framework/sessions storage/framework/views storage/logs

# Before the migrations run: the database file, on the volume.
COPY --chmod=755 docker/10-database.sh /etc/entrypoint.d/10-database.sh
COPY docker/unscripted.conf /etc/nginx/server-opts.d/unscripted.conf
