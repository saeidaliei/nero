# Nero documentation site

This is the Hugo site that publishes Nero's canonical `docs/` directory to:

**https://saeidaliei.github.io/nero/**

The Markdown files under `../docs/` are the source of truth. `scripts/sync_docs.py`
generates the Hugo content tree at build time so the documentation is not
maintained in two formats.

## Local preview

Install Hugo 0.164.x, then:

```bash
python3 docs-site/scripts/sync_docs.py
hugo server --source docs-site --buildDrafts
```

## GitHub Pages

The repository workflow at `.github/workflows/docs.yml` builds the site with Hugo
and deploys it using GitHub Pages' artifact-based deployment. In repository
Settings → Pages, select **GitHub Actions** as the publishing source.
