# The whole app (pages, components, the games' WebAssembly), for a host that runs containers
# (Coolify). Outpost's game server is its own image: outpost/server/Dockerfile.
#
#   docker build -t unscripted-demo .
#   docker run -p 8080:8080 -v demo-data:/var/www/html/storage/app/data unscripted-demo
#
# nginx and PHP-FPM (serversideup/php), on 8080. The database is SQLite on the volume at
# storage/app/data, and the app key's made there on first start (or set APP_KEY); migrations
# run on every start. Set APP_URL, and OUTPOST_SERVER_URL for Outpost's game server. Calls go over HTTP (no Reverb), so #[Live]
# boards poll.
FROM serversideup/php:8.4-fpm-nginx
EXPOSE 8080

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
    && mkdir -p storage/app/data storage/framework/cache storage/framework/sessions storage/framework/views storage/logs \
    && ln -s storage/app/data/.env .env

# Before the migrations run: the database file and an app key, on the volume.
COPY --chmod=755 docker/10-database.sh /etc/entrypoint.d/10-database.sh
COPY docker/unscripted.conf /etc/nginx/server-opts.d/unscripted.conf
