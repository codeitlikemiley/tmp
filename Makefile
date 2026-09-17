.PHONY: whitepaper whitepaper-clean fmt check clippy test integration ci

whitepaper:
	bash scripts/render-whitepaper.sh

whitepaper-clean:
	rm -rf docs/whitepaper/dist

fmt:
	cargo fmt --all -- --check

check:
	cargo check --workspace --locked --all-targets --all-features

clippy:
	cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

test:
	cargo test --workspace --locked --lib --bins

integration:
	cargo test -p tmp --locked --test e2e_tier1 --test e2e_tier2 --test e2e_tier3 --test e2e_tier4

ci: fmt check clippy test integration

