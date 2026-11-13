#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
# Set RUSTC/CARGO_HOME only if using a separate writable toolchain cache.
cargo build --locked -p paket-game --profile web-release --target wasm32-unknown-unknown
mkdir -p web/public/game web/public/mesh web/public/assets/fonts web/public/osm
wasm-bindgen --target web --remove-name-section --remove-producers-section --out-dir web/public/game --out-name paket_game target/wasm32-unknown-unknown/web-release/paket-game.wasm
cp assets/fonts/FiraMono-Medium.ttf assets/fonts/FiraMono-LICENSE web/public/assets/fonts/
cp assets/fonts/NotoSansJP-Variable.ttf assets/fonts/NotoSansJP-LICENSE web/public/assets/fonts/
if [ -f assets/landing/delivery-day.webp ]; then cp assets/landing/delivery-day.webp web/public/assets/; fi
cp ../mesh/packages/mesh-transport/transport-wasm/meta_mesh.js ../mesh/packages/mesh-transport/transport-wasm/meta_mesh_bg.wasm web/public/mesh/
# Public OSM geometry for curated starter cities avoids first-load Overpass delays.
python3 - <<'PY'
from pathlib import Path
import shutil
for p in Path('cache/osm').glob('*.json'):
    if p.name.startswith(('52.48950_13.39080_', '40.72650_-73.98150_', '-23.55050_-46.68920_', '35.66160_139.66820_', '-33.93350_18.41120_', '-33.88450_151.21330_')):
        shutil.copy2(p, Path('web/public/osm')/p.name)
PY
(cd web && npm run check && npm run build)
# Workers assets have a 25 MiB per-file limit. Serve the precompressed module at its normal URL.
python3 - <<'PY'
from pathlib import Path
import gzip
for p in Path('web/dist').rglob('*.wasm'):
    if p.stat().st_size > 24*1024*1024:
        out=p.with_suffix('.wasm.gz')
        out.write_bytes(gzip.compress(p.read_bytes(), compresslevel=9, mtime=0))
        if out.stat().st_size > 25*1024*1024: raise RuntimeError('Compressed module exceeds asset limit')
        p.unlink()
PY
