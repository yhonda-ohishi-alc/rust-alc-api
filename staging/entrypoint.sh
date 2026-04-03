#!/bin/bash
set -e

# Wait for postgres sidecar to be ready
echo "Waiting for PostgreSQL..."
until pg_isready -h localhost -p 5432 -U postgres 2>/dev/null; do
  sleep 1
done
echo "PostgreSQL is ready"

# Run migrations
echo "Running migrations..."
DATABASE_URL="postgresql://postgres:staging@localhost:5432/postgres?options=-c search_path=alc_api" \
  /usr/local/bin/migrate

echo "Migrations completed, starting app..."

# Start the app with DATABASE_URL pointing to local postgres
export DATABASE_URL="postgresql://postgres:staging@localhost:5432/postgres?options=-c search_path=alc_api"
exec /usr/local/bin/rust-alc-api
