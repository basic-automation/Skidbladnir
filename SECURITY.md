# Security policy

## Reporting a vulnerability

Please report security problems privately, not in a public issue. Open this repository's
[Security tab](https://github.com/basic-automation/Skidbladnir/security) and click
**Report a vulnerability**. Only you and the repository's maintainers can see the
report. There is no email address for security reports; that form is the channel.

Include what you can of:

- the version line from **App settings** (the gear at the bottom of the format rail),
  which names the edition and the encoder versions;
- your operating system and architecture, and how you installed Skidbladnir;
- the steps, and the file or files that show the problem;
- what happened, and what an attacker could do with it.

You will get a reply as soon as possible. A fix ships as a new release. Once it is out,
the report is published as an advisory on the Security tab, with credit to you if you
want it.

## Supported versions

Only the latest release gets security fixes. Copies from 0.8.0 onwards offer each new
release in a banner when they start. Older copies have no updater, so moving off them is
a manual download, once. The retired Electron app (v0.0.1 to v0.4.3, from 2019) gets no
fixes.

## Scope

Skidbladnir opens image files from anywhere, and it reads most formats with C and C++
libraries: libjpeg-turbo, libwebp, libavif with libaom, libjxl, and libheif with
libde265. These are in scope:

- An image file that crashes Skidbladnir, hangs it, or makes it read or write memory it
  should not.
- Any way to make a conversion write outside the destination you chose, or over the
  source file.
- Any way to make the updater install a file that was not signed with the project's key
  (see below), or to get past its signature check.
- Anything in the release pipeline: the GitHub Actions workflows, the build scripts, the
  pinned dependencies (`third_party/`, `Cargo.lock`, `frontend/package-lock.json`), and
  the `updater` branch with its manifests. For example, a way to get code into a
  release or a manifest without a commit on this repository.

A bug in one of the bundled libraries is in scope when an image can reach it through
Skidbladnir. Report it here and to the library. The fix for Skidbladnir is a release
that pins the library's fixed version.

These are not security problems:

- The installers are not code-signed with an Apple or Microsoft certificate, so Windows
  SmartScreen and macOS Gatekeeper warn about them. That is known.
- Output that differs from the reference tool's. Use the **Parity mismatch** issue form.

## Checking an update or a download

Every update is signed with the project's [minisign](https://jedisct1.github.io/minisign/)
key. The app carries the public key, base64-encoded, as `plugins.updater.pubkey` in
[`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json), and it refuses an update whose
signature does not match. Both editions use the same key, ID `E098D9770764D621`:

```text
RWQh1mQHd9mY4KLXq4PSksQm0NeZiJuCwpNmkgJhO09j4PJ3PQo6jWbN
```

The signatures are in the update manifests on the
[`updater` branch](https://github.com/basic-automation/Skidbladnir/tree/updater):
`latest.json` for the standard edition and `latest-gpl.json` for the GPL edition. Each
release's manifests stay in that branch's history. A manifest has one entry per platform,
and each entry's `signature` is a minisign signature, base64-encoded:

| Entry | Signed file |
|---|---|
| `windows-x86_64-nsis` | the Windows installer, `*_x64-setup.exe` |
| `linux-x86_64-appimage` | the Linux AppImage |
| `linux-x86_64-deb` | the Linux `.deb` |
| `darwin-aarch64-app` | the macOS app archive for Apple Silicon, `*_aarch64.app.tar.gz` |
| `darwin-x86_64-app` | the macOS app archive for Intel, `*_x64.app.tar.gz` |

The macOS `.dmg` files are not signed this way. `latest.json` lists the newest release;
for an older one, take the manifest from that release's commit on the branch. For
example, to check the standard edition's 1.0.0 AppImage:

```sh
curl -sL https://raw.githubusercontent.com/basic-automation/Skidbladnir/updater/latest.json \
  | jq -r '.platforms["linux-x86_64-appimage"].signature' | base64 -d > appimage.minisig
minisign -V -P RWQh1mQHd9mY4KLXq4PSksQm0NeZiJuCwpNmkgJhO09j4PJ3PQo6jWbN \
  -m Skidbladnir_1.0.0_amd64.AppImage -x appimage.minisig
```

A good signature means the file was signed with the project's key. It does not make
Windows or macOS trust the installer; they still warn when you open it.
