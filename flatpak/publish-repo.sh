#!/usr/bin/env bash
# Turns a signed Flatpak repo built by flatpak-builder into the site served
# from GitHub Pages: the repo itself plus the .flatpakrepo and .flatpakref
# files people install from.
#   publish-repo.sh REPO_DIR GPG_HOMEDIR KEY_ID OUT_DIR BASE_URL
set -euo pipefail
repo=$1 gpghome=$2 key=$3 out=$4 base=${5%/}
app=io.github.rafaellgc.Renome

flatpak build-update-repo --gpg-sign="$key" --gpg-homedir="$gpghome" \
  --title=Renome --default-branch=stable --generate-static-deltas --prune "$repo"

rm -rf "$out"
mkdir -p "$out"
cp -a "$repo" "$out/repo"
gpgkey=$(gpg --homedir "$gpghome" --export "$key" | base64 -w0)

cat > "$out/renome.flatpakrepo" <<EOF
[Flatpak Repo]
Title=Renome
Url=$base/repo/
Homepage=https://github.com/rafaellgc/Renome
Comment=Batch file renamer
GPGKey=$gpgkey
EOF

cat > "$out/$app.flatpakref" <<EOF
[Flatpak Ref]
Name=$app
Branch=stable
Title=Renome
Url=$base/repo/
SuggestRemoteName=renome
Homepage=https://github.com/rafaellgc/Renome
IsRuntime=false
GPGKey=$gpgkey
RuntimeRepo=https://dl.flathub.org/repo/flathub.flatpakrepo
EOF

cat > "$out/index.html" <<EOF
<!doctype html>
<meta charset="utf-8">
<title>Renome Flatpak repository</title>
<p>Flatpak repository for <a href="https://github.com/rafaellgc/Renome">Renome</a>.</p>
<pre>flatpak install --user $base/$app.flatpakref</pre>
EOF
touch "$out/.nojekyll"
