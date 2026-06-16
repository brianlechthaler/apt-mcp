.PHONY: test lint fmt coverage build docker-test docker-lint docker-coverage docker-build

test:
	docker compose run --rm test

lint:
	docker compose run --rm lint

fmt:
	docker compose run --rm dev cargo fmt

coverage:
	docker compose run --rm coverage

build:
	docker compose build

docker-test: test
docker-lint: lint
docker-coverage: coverage
docker-build: build
