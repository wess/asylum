# macOS signing provisioning

One-time setup, run on a Mac signed into the Apple Developer account (the
release machine). The outcome is six GitHub repository secrets; the release
workflow and `packaging/macos.sh` pick them up automatically — no code
changes. Until they exist, builds fall back to unsigned.

## 1. Membership

Enroll the account at developer.apple.com/programs ($99/yr; approval can take
a day or two). Note the 10-character Team ID from the Membership details page.
Only the Account Holder role can create Developer ID certificates.

## 2. Developer ID Application certificate

Fastest path — Xcode: Settings → Accounts → select the account → Manage
Certificates… → **+** → "Developer ID Application". The certificate and its
private key land in the login keychain.

Portal alternative: Keychain Access → Certificate Assistant → Request a
Certificate From a Certificate Authority (saved to disk), upload the CSR at
developer.apple.com → Certificates → **+** → Developer ID Application,
download the `.cer`, and double-click it into the login keychain.

## 3. Export the .p12

Keychain Access → My Certificates → "Developer ID Application: <Name>
(<TEAMID>)" → export as `developerid.p12` with a strong password, then:

```sh
base64 -i developerid.p12 > developerid.p12.b64
```

## 4. Notarization credentials

Generate an app-specific password at account.apple.com → Sign-In and Security
→ App-Specific Passwords (name it e.g. `asylum notarize`). `AC_USERNAME` is
the Apple ID email, `AC_TEAM_ID` the Team ID from step 1.

## 5. Set the repository secrets

```sh
gh secret set MACOS_CERT_P12      --repo wess/asylum < developerid.p12.b64
gh secret set MACOS_CERT_PASSWORD --repo wess/asylum --body '<p12 password>'
gh secret set MACOS_SIGN_IDENTITY --repo wess/asylum --body 'Developer ID Application: <Name> (<TEAMID>)'
gh secret set AC_USERNAME         --repo wess/asylum --body '<apple id email>'
gh secret set AC_PASSWORD         --repo wess/asylum --body '<app-specific password>'
gh secret set AC_TEAM_ID          --repo wess/asylum --body '<TEAMID>'
```

The identity string must match `security find-identity -v -p codesigning`
output exactly. `MACOS_CERT_PASSWORD` doubles as the throwaway CI keychain
password (`packaging/macos.sh`).

## 6. Verify

Local dry run on the release Mac (the cert is already in the login keychain,
so `MACOS_CERT_P12` can stay unset):

```sh
export MACOS_SIGN_IDENTITY='Developer ID Application: <Name> (<TEAMID>)'
export AC_USERNAME='<apple id email>' AC_PASSWORD='<app-specific password>' AC_TEAM_ID='<TEAMID>'
cargo install cargo-bundle --locked
(cd crates/app && cargo bundle --release --format osx --target aarch64-apple-darwin)
packaging/macos.sh aarch64-apple-darwin macos-arm64
spctl -a -t open --context context:primary-signature -vv dist/asylum-macos-arm64.dmg   # accepted
xcrun stapler validate dist/asylum-macos-arm64.dmg                                     # The validate action worked
```

Then prove the pipeline: bump the version, push to `main`, and check the
release job logs for "signing"/"notarizing" instead of "no signing identity
set".

## 7. Clean up

```sh
rm -P developerid.p12 developerid.p12.b64
```

Key material never belongs on disk longer than the setup takes.
