# Security

## Reporting a vulnerability

Report a vulnerability privately, through GitHub's private vulnerability reporting: the
repository's **Security** tab, then **Report a vulnerability**. Do not open a public issue for it.

Say what is affected, how to reproduce it, and what it lets someone do. The owner answers as soon
as they can; there is no bounty.

## What is supported

Before 1.0, fixes land on `main` and in the next release; earlier releases get none.

## What matters most

- Files that the engine reads from outside the program, cooked data, saves, scene files, source
  assets and their metadata: a damaged or hostile file must give an error, never a crash, a hang,
  memory exhausted or code run ([ADR-0096](docs/adr/0096-the-files-the-engine-reads.md),
  [ADR-0102](docs/adr/0102-a-damaged-file-gives-an-error.md)).
- The development channel and the MCP server, which listen on the machine alone, behind a token
  ([ADR-0122](docs/adr/0122-the-development-channel.md)): anything that reaches them from elsewhere,
  or without the token.
- The engine and the editor send nothing over the network of their own accord
  ([ADR-0207](docs/adr/0207-what-slimeengine-is-to-become.md)): anything that does.
