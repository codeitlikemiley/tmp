.PHONY: whitepaper whitepaper-clean

whitepaper:
	bash scripts/render-whitepaper.sh

whitepaper-clean:
	rm -rf docs/whitepaper/dist
