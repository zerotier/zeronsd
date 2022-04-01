## Cutting a release

If any of this doesn't work, alert @erikh immediately.

- Run `make clean` and let it run as root.
- Edit `Cargo.toml` to update the version string near the top of the file.
- Put a token attached to a central instance into `test-token.txt`. This file is in `.gitignore` and should not be attached to networks you care about.
- `make test-integration`. This will create and join your host to several networks.
- Commit and push main: `git commit -a -s -m "zeronsd v<version>" && git push <remote> main`
- Tag git: `git tag v<version>`. The `v` is important here. Delete the tag if you created a non-`v` tag.
- Push the tag: `git push <remote> v<version>`
- Push cargo: `cargo publish`. (Get @erikh involved if you need to)
- `make test-packages`. Read the output to ensure it passes. It will also build the packages.
- Push docker images: `make docker-image-push`. This will also tag `latest` images.
- Edit the release tag, it'll be [here](https://github.com/zerotier/zeronsd/releases).
  - In the `target` directory, there will be several files. Push them to the release.
    - `zeronsd_*.deb` is for debian/ubuntu systems.
    - `zeronsd-*.rpm` is for RedHat systems.
- Windows: Start up a windows VM. You should have the following tools:
  - [Rust Compiler](https://rustup.rs) -- note that this yields an unsigned binary. Just install it, wimp.
    - Follow the printed instructions on getting the [C++ runtime](https://visualstudio.microsoft.com/visual-cpp-build-tools/). You will need it.
    - If cargo doesn't work after this, you've done it wrong.
  - [Strawberry Perl](https;//www.strawberryperl.com) needs to be installed to build the openssl dependency.
  - [WIX Toolset](https://github.com/wixtoolset/wix3/releases/tag/wix3112rtm) also needs to be installed.
  - Finally, set up some kind of clone of the tag, or VM shared folders.
  - Run `cargo wix -L -ext -L WixFirewallExtension -C -ext -C WixFirewallExtension` in a windows terminal.
    - The resulting installer will be in `target/wix/*.msi`. You do not need the other files.
- OS X: Get permissions from Joseph to commit to [homebrew-tap](https://github.com/zerotier/homebrew-tap) or submit a PR.
  - Edit `Formula/zeronsd.rb` and change the following attributes:
    - version
    - git SHA
  - Commit it and run `make test-packages`. This will test the installation of your package on linuxbrew and mac homebrew.
