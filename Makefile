.PHONY: check test-contracts build deploy-sepolia run-cluster smoke-test

check:
	cargo check --manifest-path Cargo.toml
	cd contracts && npx hardhat compile

test-contracts:
	cd contracts && npx hardhat test

build:
	cargo build --release --manifest-path Cargo.toml

deploy-sepolia:
	cd contracts && npm run deploy:sepolia

run-cluster:
	@test -f cluster/.env || (echo "Copy cluster/.env.example to cluster/.env and fill in values" && exit 1)
	env $$(cat cluster/.env | xargs) ./target/release/polyclaw-cluster

smoke-test:
	@bash scripts/smoke-test.sh
