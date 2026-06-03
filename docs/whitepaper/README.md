# TMP White Paper

Edit the white paper source here:

```text
docs/whitepaper/tool-mapping-protocol.md
```

Regenerate the distributable PDF from the repository root:

```bash
make whitepaper
```

The generated PDF is written to:

```text
docs/whitepaper/dist/tool-mapping-protocol-whitepaper.pdf
```

The render path uses Pandoc with Typst as the PDF engine. If either tool is missing, the script will fail with a dependency message.

Supporting evaluation artifacts:

```text
docs/whitepaper/benchmark-plan.md
docs/whitepaper/benchmark-runs.schema.json
```

Use these to track whether TMP reduces tool calls, token usage, hallucinated operations, and time-to-correct-action across baseline and TMP-assisted runs.
