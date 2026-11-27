# rrbs-tickets-rs - A Ticket Management Tool

> **Note:** This project is an independent continuation (hard fork) of [TheBiochemic/tickets-rs](https://github.com/TheBiochemic/tickets-rs), originally created by Robert Lang (Biochemic), as the original repository is no longer actively maintained. Licensed under the **GPLv3**.

`rrbs-tickets-rs` is a desktop application built with `egui` for managing and modifying tickets, issues, tasks, and TODOs. Its goal is to combine multiple ticket sources via modular adapters or keep everything locally in one place.

![App Screenshot](assets/screenshot_app.png)

Currently supported UI language is **English**.

---

## Features

* Written in **100% safe Rust**
* Manage, edit, and organize tickets from different sources in one place
* Add tags to tickets for quick filtering and sorting
* Create categories ("Buckets") to group tickets
* Build custom filters to view specific combinations of tickets across multiple adapters
* Write and preview descriptions using CommonMark (Markdown)
* Customize appearance on the fly (including font size and custom colors)
* Add and remove ticket sources ("Adapters") at runtime without restarting
* Includes a built-in setup wizard to get started quickly

---

## Currently Supported Adapters

* **local:** Tickets are stored on the local drive in an SQLite database. Can also be placed on a network share (e.g., via `sshfs`), though concurrent file locking has not been extensively tested.
* **github:** Reads and caches issues from GitHub repositories (refreshed every 5 minutes on request, read-only). Requires a GitHub Personal Access Token. Supports displaying repositories from your own account as well as other public accounts. Currently displays basic issue states without comments or pull requests.

---

## Getting Started (Build from Source)

Since the application is written in Rust, you need a working Rust toolchain (`rustc` and `cargo`).

1. **Install Rust** (if not already installed): Follow the [official Rust installation guide](https://doc.rust-lang.org/cargo/getting-started/installation.html).
2. **Clone the repository:**
   ```bash
   git clone [https://github.com/uralrabo/rrbs-tickets-rs.git](https://github.com/uralrabo/rrbs-tickets-rs.git)
   cd rrbs-tickets-rs


## Contributors

The original project, tickets-rs, was created by Robert Lang (Biochemic).

This independent continuation is maintained by Ralf Boehm (rrbs), who is further developing it as a personal learning project.

## License

This project is licensed under the GNU General Public License v3.0.
See the `LICENSE` file for the full license text.
