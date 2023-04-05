<a href="https://developers.diem.com">
	<img width="200" src="./.assets/diem.png" alt="Diem Logo" />
</a>

---

[![License](https://img.shields.io/badge/license-Apache-green.svg)](LICENSE)
[![Discord chat](https://img.shields.io/discord/903339070925721652.svg?logo=discord&style=flat-square)](https://discord.gg/epNwRT2wcd)


# The Move Language

Move is a new programmable platform for blockchains and other applications where safety and correctness are paramount. It is an executable bytecode language designed to provide safe and verifiable transaction-oriented computation. The language features a strong type system with linear resource types, runtime checks, and formal verification.

## Quickstart

### Build the [Docker](https://www.docker.com/community/open-source/) Image for the Command Line Tool

```
docker build -t move/cli -f docker/move-cli/Dockerfile .
```

### Build a Test Project

```
cd ./language/documentation/tutorial/step_1/BasicCoin
docker run -v `pwd`:/project move/cli build
```

## Community
* Join us on the [Move Discord](https://discord.gg/M95qX3KnG8).
* Browse code and content from the community at [awesome-move](https://github.com/MystenLabs/awesome-move).

## License

Move is licensed as [Apache 2.0](https://github.com/diem/diem/blob/main/LICENSE).
