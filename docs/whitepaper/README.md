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

Diagram assets live in:

```text
docs/whitepaper/assets/
```

The SVG files are the editable diagram sources. PNG versions are checked in for the PDF renderer because the local Pandoc and Typst path does not require a separate Mermaid or SVG conversion dependency.

The render path uses Pandoc with Typst as the PDF engine. If either tool is missing, the script will fail with a dependency message.

Supporting evaluation artifacts:

```text
docs/whitepaper/benchmark-plan.md
docs/whitepaper/benchmark-runs.schema.json
```

Use these to track whether TMP reduces tool calls, token usage, hallucinated operations, and time-to-correct-action across baseline and TMP-assisted runs.

The benchmark plan also includes output-policy scenarios for RTK-style command-output reduction. Those scenarios can compare raw baseline output, TMP built-in output shaping, and a combined TMP plus RTK mode where TMP resolves the operation and RTK compresses CLI output.

The white paper also sketches a future `tmp generate rtk <operation-id>` workflow. That workflow should draft RTK-compatible filters for unsupported local commands, keep them unverified by default, and require sample-based verification before use.
