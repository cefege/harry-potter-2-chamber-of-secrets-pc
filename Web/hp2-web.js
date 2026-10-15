'use strict';

(function () {
  const MARKER = '.import-complete.json';
  const DATA_ROOT = '/hp2data';
  const USER_ROOT = '/hp2home/harry-potter-2/User';
  const HOME_PREFIX = '/hp2home';
  const LOG_LINES = 20000;
  const params = new URLSearchParams(location.search);
  const devMode = params.has('devdata');
  const smokeMode = ['shell', 'boot', 'launcher'].indexOf(params.get('smoke')) >= 0
    ? params.get('smoke') : null;
  const canvas = document.getElementById('canvas');
  const statusEl = document.getElementById('status');
  const logEl = document.getElementById('log');
  const logToggle = document.getElementById('log-toggle');
  const stageEl = document.getElementById('stage');
  const fullscreenToggle = document.getElementById('fullscreen-toggle');
  const backupSave = document.getElementById('backup-save');
  const backupRestore = document.getElementById('backup-restore');
  const backupInput = document.getElementById('backup-input');
  const COOP_STATUS =
    'This page must be served with COOP/COEP headers (python3 Build/serve_web.py).';
  const MEMORY64_PROBE = new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0, 5, 3, 1, 4, 1]); // (module (memory i64 1))
  const UNSUPPORTED_BROWSER = 'This browser cannot run the game. Use Chrome, Edge or Firefox on a computer, '
    + 'or Chrome/Firefox on Android. iPhone and iPad are not supported.';
  const RETAIL_FOLDER_LEAD = 'To play, choose the folder where the game is installed on this computer.';
  const RETAIL_FOLDER_HINTS = [
    'Look for the folder named \u201CHarry Potter and the Chamber of Secrets\u201D. '
      + 'It contains the folders Maps, System, Textures, Sounds, Music and Help.',
    'Can\u2019t find it? Search your computer for HP2_Master.utx. It is inside the Textures folder, '
      + 'so choose the folder one level above Textures.',
    'Your browser may say it will \u201Cupload\u201D the files. They stay on this device; nothing is sent anywhere.',
  ];
  let currentModule = null;
  let started = false;
  let syncInFlight = false;
  let smokeReported = false;
  let nextRequestId = 1;
  const pendingPickers = new Map();
  let folderOverlay = null;

  function setStatus(text) { statusEl.textContent = text; }

  function logLine(text) {
    const atBottom = logEl.scrollTop + logEl.clientHeight >= logEl.scrollHeight - 4;
    logEl.textContent += text + '\n';
    const lines = logEl.textContent.split('\n');
    if (lines.length > LOG_LINES) logEl.textContent = lines.slice(-LOG_LINES).join('\n');
    if (atBottom) logEl.scrollTop = logEl.scrollHeight;
  }

  function logToBoth(text) {
    logLine(text);
    console.log(text);
    if (String(text).indexOf('HP2Launcher: ready') >= 0) {
      // The launcher is on screen and speaks for itself from here.
      if (statusEl.textContent === 'Starting launcher…') setStatus('');
      if (smokeMode === 'launcher') reportSmoke('ready', 'in-engine launcher rendered its first frame');
    }
  }

  function reportSmoke(stage, detail) {
    if (!smokeMode || smokeReported) return;
    smokeReported = true;
    const report = { mode: smokeMode, stage: stage, detail: detail,
      frames: 0, log: logEl.textContent.split('\n').slice(-400).join('\n') };
    if (smokeMode === 'launcher' && stage === 'ready') {
      try { report.screenshot = canvas.toDataURL('image/png'); }
      catch (err) { report.screenshotError = String(err); }
    }
    fetch('/__hp2_smoke', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(report),
    }).catch(function (err) { console.error('smoke report failed: ' + err); });
  }

  const CANONICAL_ROOTS = {
    system: 'System', maps: 'Maps', textures: 'Textures', sounds: 'Sounds',
    music: 'music', help: 'Help',
  };
  const SUFFIX = 'System/Default.ini';

  function normalizeDataPath(rel) {
    const parts = String(rel).replace(/\\/g, '/').split('/').filter(Boolean);
    if (!parts.length) return null;
    if (parts.length === 1) {
      return parts[0] === 'overlay-manifest.json' || parts[0] === 'overlay-checksums.txt'
        ? parts[0] : null;
    }
    const root = parts[0].toLowerCase();
    if (parts.length === 2) {
      const canonical = CANONICAL_ROOTS[root];
      if (!canonical || parts[1].toLowerCase().endsWith('.scc')) return null;
      return canonical + '/' + parts[1];
    }
    if (root === 'system' && parts[1].toLowerCase() === 'cutscenes') {
      return ['System', 'Cutscenes'].concat(parts.slice(2)).join('/');
    }
    return null;
  }

  const NOT_GAME_FOLDER = 'This folder does not contain the game. Choose the folder that contains '
    + 'Maps, System and Textures (with HP2_Master.utx inside Textures).';

  // The browser prefixes every picked path with the chosen folder's own name,
  // and players may pick a folder above the game. The game root is the
  // shallowest directory holding Textures/HP2_Master.utx, a file only HP2
  // ships (HP1 also has a System folder); null when there is none.
  const HP2_MARKER = 'textures/hp2_master.utx';
  function findGameRoot(paths) {
    let best = null;
    for (const path of paths) {
      const lower = path.toLowerCase();
      if (lower !== HP2_MARKER && !lower.endsWith('/' + HP2_MARKER)) continue;
      const root = path.slice(0, path.length - HP2_MARKER.length).replace(/\/$/, '');
      if (best === null || root.split('/').length < best.split('/').length) best = root;
    }
    return best;
  }

  function findDataRoot(paths) {
    const normalized = paths.map((path) => String(path).replace(/\\/g, '/'));
    const suffix = SUFFIX.toLowerCase();
    const matches = normalized.filter((path) => {
      const lower = path.toLowerCase();
      return lower === suffix || lower.endsWith('/' + suffix);
    }).sort((a, b) => a.length - b.length);
    if (!matches.length) throw new Error('No System/Default.ini found in the selected folder.');
    const shortest = matches[0];
    const root = shortest.length === suffix.length
      ? '' : shortest.slice(0, shortest.length - suffix.length - 1);
    return { root: root, relativize: (path) =>
      root && path.startsWith(root + '/') ? path.slice(root.length + 1) : path };
  }

  async function writeToOpfs(rootDir, relative, data) {
    const parts = relative.split('/');
    let dir = rootDir;
    for (let i = 0; i < parts.length - 1; ++i) {
      dir = await dir.getDirectoryHandle(parts[i], { create: true });
    }
    const handle = await dir.getFileHandle(parts[parts.length - 1], { create: true });
    const stream = await handle.createWritable();
    await stream.write(data);
    await stream.close();
  }

  async function clearOpfsDirectory(parent, name) {
    try { await parent.removeEntry(name, { recursive: true }); }
    catch (err) { if (!err || err.name !== 'NotFoundError') throw err; }
  }

  async function markDirectory(dir, files, bytes) {
    const marker = { files: files, bytes: bytes, importedAt: new Date().toISOString() };
    await writeToOpfs(dir, MARKER, new Blob([JSON.stringify(marker)], {
      type: 'application/json',
    }));
  }

  async function importEntries(entries) {
    const dataRoot = findDataRoot(entries.map((entry) => entry.path));
    const opfs = await navigator.storage.getDirectory();
    await clearOpfsDirectory(opfs, 'prototype');
    const target = await opfs.getDirectoryHandle('prototype', { create: true });
    let written = 0;
    let bytes = 0;
    for (const entry of entries) {
      const path = normalizeDataPath(dataRoot.relativize(entry.path));
      if (!path) continue;
      let data;
      try { data = await entry.read(); }
      catch (err) { throw new Error(entry.path + ': ' + err); }
      await writeToOpfs(target, path, data);
      ++written;
      bytes += data.byteLength;
      if ((written & 63) === 0) await new Promise((resolve) => setTimeout(resolve, 0));
    }
    await markDirectory(target, written, bytes);
    setStatus('Prototype data imported (' + written + ' files).');
    logToBoth('Imported prototype data: ' + written + ' files.');
  }

  async function hasMarker(opfs, kind) {
    try {
      const dir = await opfs.getDirectoryHandle(kind);
      await dir.getFileHandle(MARKER);
      return true;
    } catch (err) { return false; }
  }

  async function migrateLegacyPrototype(opfs) {
    if (await hasMarker(opfs, 'prototype')) return;
    let legacy;
    try { legacy = await opfs.getDirectoryHandle('hp2-data'); }
    catch (err) { if (err && err.name === 'NotFoundError') return; throw err; }
    let hasLegacyMarker = false;
    try { await legacy.getFileHandle(MARKER); hasLegacyMarker = true; }
    catch (err) { if (!err || err.name !== 'NotFoundError') throw err; }
    if (!hasLegacyMarker) return;
    await clearOpfsDirectory(opfs, 'prototype');
    const prototype = await opfs.getDirectoryHandle('prototype', { create: true });
    const files = await listOpfsDataFiles(legacy, '');
    let bytes = 0;
    for (const file of files) {
      const data = await file.handle.getFile().then((f) => f.arrayBuffer());
      await writeToOpfs(prototype, file.path, data);
      bytes += data.byteLength;
    }
    await markDirectory(prototype, files.length, bytes);
    await clearOpfsDirectory(opfs, 'hp2-data');
    logToBoth('Migrated existing browser data into the prototype source.');
  }

  async function listOpfsDataFiles(dir, prefix) {
    const files = [];
    for await (const [name, handle] of dir.entries()) {
      if (name === MARKER && !prefix) continue;
      if (handle.kind === 'directory') {
        files.push(...await listOpfsDataFiles(await dir.getDirectoryHandle(name), prefix + name + '/'));
      } else {
        files.push({ handle: handle, path: prefix + name });
      }
    }
    return files;
  }

  function mkdirTree(fs, path) {
    let current = '';
    for (const part of path.split('/').filter(Boolean)) {
      current += '/' + part;
      try { fs.mkdir(current); } catch (err) { if (!fs.analyzePath(current).exists) throw err; }
    }
  }

  function removeFsTree(fs, path) {
    let stat;
    try { stat = fs.stat(path); } catch (err) { return; }
    if (fs.isDir(stat.mode)) {
      for (const name of fs.readdir(path)) {
        if (name !== '.' && name !== '..') removeFsTree(fs, path + '/' + name);
      }
      fs.rmdir(path);
    } else fs.unlink(path);
  }

  // Every regular file under root as {path relative to root, data}, keeping
  // only names for which keep(name) is true.
  function readFsTree(fs, root, keep) {
    const files = [];
    function visit(directory, relative) {
      for (const name of fs.readdir(directory)) {
        if (name === '.' || name === '..') continue;
        const path = directory + '/' + name;
        const rel = relative ? relative + '/' + name : name;
        if (fs.isDir(fs.stat(path).mode)) visit(path, rel);
        else if (keep(name)) files.push({ path: rel, data: fs.readFile(path) });
      }
    }
    visit(root, '');
    return files;
  }

  // Store-only (method 0) zip, enough for save backups without a library.
  const CRC32_TABLE = (function () {
    const table = new Uint32Array(256);
    for (let n = 0; n < 256; ++n) {
      let c = n;
      for (let k = 0; k < 8; ++k) c = c & 1 ? 0xEDB88320 ^ (c >>> 1) : c >>> 1;
      table[n] = c >>> 0;
    }
    return table;
  })();

  function crc32(bytes) {
    let crc = 0xFFFFFFFF;
    for (let i = 0; i < bytes.length; ++i) crc = CRC32_TABLE[(crc ^ bytes[i]) & 0xFF] ^ (crc >>> 8);
    return (crc ^ 0xFFFFFFFF) >>> 0;
  }

  const ZIP_LOCAL = 0x04034b50;
  const ZIP_CENTRAL = 0x02014b50;
  const ZIP_END = 0x06054b50;
  const ZIP_UTF8 = 0x0800;
  const NOT_A_BACKUP = 'This file is not a Harry Potter 2 save backup.';

  function buildStoredZip(entries) {
    const encoder = new TextEncoder();
    const now = new Date();
    const dosTime = (now.getHours() << 11) | (now.getMinutes() << 5) | (now.getSeconds() >> 1);
    const dosDate = ((now.getFullYear() - 1980) << 9) | ((now.getMonth() + 1) << 5) | now.getDate();
    const records = [];
    const directory = [];
    let offset = 0;
    for (const entry of entries) {
      const name = encoder.encode(entry.path);
      const crc = crc32(entry.data);
      const size = entry.data.length;
      const local = new DataView(new ArrayBuffer(30));
      local.setUint32(0, ZIP_LOCAL, true);
      local.setUint16(4, 20, true);
      local.setUint16(6, ZIP_UTF8, true);
      local.setUint16(10, dosTime, true);
      local.setUint16(12, dosDate, true);
      local.setUint32(14, crc, true);
      local.setUint32(18, size, true);
      local.setUint32(22, size, true);
      local.setUint16(26, name.length, true);
      records.push(local, name, entry.data);
      const central = new DataView(new ArrayBuffer(46));
      central.setUint32(0, ZIP_CENTRAL, true);
      central.setUint16(4, 20, true);
      central.setUint16(6, 20, true);
      central.setUint16(8, ZIP_UTF8, true);
      central.setUint16(12, dosTime, true);
      central.setUint16(14, dosDate, true);
      central.setUint32(16, crc, true);
      central.setUint32(20, size, true);
      central.setUint32(24, size, true);
      central.setUint16(28, name.length, true);
      central.setUint32(42, offset, true);
      directory.push(central, name);
      offset += 30 + name.length + size;
    }
    const directorySize = directory.reduce((sum, part) => sum + part.byteLength, 0);
    const end = new DataView(new ArrayBuffer(22));
    end.setUint32(0, ZIP_END, true);
    end.setUint16(8, entries.length, true);
    end.setUint16(10, entries.length, true);
    end.setUint32(12, directorySize, true);
    end.setUint32(16, offset, true);
    return new Blob(records.concat(directory, [end]), { type: 'application/zip' });
  }

  function readStoredZip(buffer) {
    const bytes = new Uint8Array(buffer);
    const view = new DataView(buffer);
    const invalid = () => new Error(NOT_A_BACKUP);
    let end = -1;
    for (let i = bytes.length - 22; i >= 0 && i >= bytes.length - 65557; --i) {
      if (view.getUint32(i, true) === ZIP_END) { end = i; break; }
    }
    if (end < 0) throw invalid();
    const decoder = new TextDecoder('utf-8', { fatal: true });
    const entries = [];
    try {
      const count = view.getUint16(end + 10, true);
      let cursor = view.getUint32(end + 16, true);
      for (let i = 0; i < count; ++i) {
        if (view.getUint32(cursor, true) !== ZIP_CENTRAL) throw invalid();
        const flags = view.getUint16(cursor + 8, true);
        const method = view.getUint16(cursor + 10, true);
        const crc = view.getUint32(cursor + 16, true);
        const size = view.getUint32(cursor + 20, true);
        const nameLength = view.getUint16(cursor + 28, true);
        const localOffset = view.getUint32(cursor + 42, true);
        if (method !== 0 || (flags & 1) || size !== view.getUint32(cursor + 24, true)) throw invalid();
        const path = decoder.decode(bytes.subarray(cursor + 46, cursor + 46 + nameLength));
        if (view.getUint32(localOffset, true) !== ZIP_LOCAL) throw invalid();
        const start = localOffset + 30 + view.getUint16(localOffset + 26, true)
          + view.getUint16(localOffset + 28, true);
        if (start + size > bytes.length) throw invalid();
        const data = bytes.slice(start, start + size);
        if (crc32(data) !== crc) throw invalid();
        entries.push({ path: path, data: data });
        cursor += 46 + nameLength + view.getUint16(cursor + 30, true) + view.getUint16(cursor + 32, true);
      }
    } catch (err) { throw invalid(); }
    return entries;
  }

  async function copyOpfsTreeToFs(kind, m) {
    const opfs = await navigator.storage.getDirectory();
    const source = await opfs.getDirectoryHandle(kind);
    const files = await listOpfsDataFiles(source, '');
    const destination = DATA_ROOT + '/' + kind;
    removeFsTree(m.FS, destination);
    mkdirTree(m.FS, destination);
    for (let i = 0; i < files.length; ++i) {
      const file = files[i];
      const data = await file.handle.getFile().then((f) => f.arrayBuffer());
      const path = destination + '/' + file.path;
      mkdirTree(m.FS, path.slice(0, path.lastIndexOf('/')));
      m.FS.writeFile(path, new Uint8Array(data));
      if ((i & 63) === 63) await new Promise((resolve) => setTimeout(resolve, 0));
    }
    return files.length;
  }

  async function copyFsTreeToOpfs(m, sourcePath, kind) {
    const files = readFsTree(m.FS, sourcePath, () => true);
    const opfs = await navigator.storage.getDirectory();
    await clearOpfsDirectory(opfs, kind);
    const destination = await opfs.getDirectoryHandle(kind, { create: true });
    let bytes = 0;
    for (let i = 0; i < files.length; ++i) {
      const file = files[i];
      await writeToOpfs(destination, file.path, file.data);
      bytes += file.data.byteLength;
      if ((i & 31) === 31) await new Promise((resolve) => setTimeout(resolve, 0));
    }
    await markDirectory(destination, files.length, bytes);
  }

  function bridgeComplete(id, ok, payload) {
    const m = currentModule;
    if (!m || typeof m._hp2_web_bridge_done !== 'function') {
      logToBoth('Web bridge callback unavailable for request ' + id);
      return;
    }
    let pointer = 0;
    try {
      const bytes = new TextEncoder().encode(payload == null ? '' : String(payload));
      pointer = m._malloc(bytes.length + 1);
      if (!pointer) throw new Error('Could not allocate web bridge payload.');
      m.HEAPU8.set(bytes, pointer);
      m.HEAPU8[pointer + bytes.length] = 0;
      // MEMORY64: _malloc is wrapped to return a Number, but this export is
      // the raw wasm function, so its const char* parameter is an i64.
      m._hp2_web_bridge_done(id, ok ? 1 : 0, BigInt(pointer));
    } finally {
      if (pointer && typeof m._free === 'function') m._free(pointer);
    }
  }

  function nextId() { return nextRequestId++; }

  // Full-page folder prompt. For retail data this is the first thing a new
  // player sees: the launcher opens it on its own when no game files are
  // installed. It stays up while the files are read, so there is always
  // something on screen saying what is happening.
  function makeFolderOverlay() {
    if (folderOverlay) return folderOverlay;
    const overlay = document.createElement('div');
    overlay.id = 'hp2-folder-overlay';
    overlay.hidden = true;
    const card = document.createElement('div');
    card.className = 'card';
    const title = document.createElement('h1');
    title.textContent = 'Harry Potter and the Chamber of Secrets';
    const lead = document.createElement('p');
    lead.className = 'lead';
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'primary';
    const progress = document.createElement('p');
    progress.className = 'progress';
    progress.hidden = true;
    const hints = document.createElement('div');
    const later = document.createElement('button');
    later.type = 'button';
    later.className = 'secondary';
    later.textContent = 'Not now';
    const input = document.createElement('input');
    input.type = 'file';
    input.multiple = true;
    input.setAttribute('webkitdirectory', '');
    input.hidden = true;
    card.append(title, lead, button, progress, hints, later);
    overlay.append(card, input);
    document.body.appendChild(overlay);
    let overlayRequest = null;

    function close() {
      overlay.hidden = true;
      button.hidden = false;
      later.hidden = false;
      progress.hidden = true;
      input.value = '';
    }
    function showProgress(text) {
      button.hidden = true;
      later.hidden = true;
      progress.hidden = false;
      progress.textContent = text;
    }

    button.addEventListener('click', function () { input.click(); });
    // Declining is not an error, so the launcher gets no message to show.
    later.addEventListener('click', function () {
      const request = overlayRequest;
      overlayRequest = null;
      close();
      if (request) bridgeComplete(request.id, false, '');
    });
    // Closing the browser's picker (or refusing its upload prompt) keeps this
    // page up so the player can simply try again.
    input.addEventListener('cancel', function () { input.value = ''; });
    input.addEventListener('change', async function () {
      const request = overlayRequest;
      if (!request || !input.files || input.files.length === 0) { input.value = ''; return; }
      overlayRequest = null;
      const selected = Array.from(input.files);
      try {
        showProgress('Reading your game files\u2026');
        const paths = selected.map((file) => (file.webkitRelativePath || file.name).replace(/\\/g, '/'));
        const root = findGameRoot(paths);
        if (root === null) throw new Error(NOT_GAME_FOLDER);
        const prefix = root ? root + '/' : '';
        const m = currentModule;
        const source = '/hp2import/source';
        removeFsTree(m.FS, source);
        mkdirTree(m.FS, source);
        // Only the game folder itself is read; anything beside it is ignored.
        const wanted = [];
        for (let i = 0; i < selected.length; ++i) {
          if (paths[i].startsWith(prefix)) wanted.push({ file: selected[i], rel: paths[i].slice(prefix.length) });
        }
        for (let i = 0; i < wanted.length; ++i) {
          const rel = wanted[i].rel;
          if (rel.split('/').some((part) => !part || part === '.' || part === '..')) {
            throw new Error('Unsafe file path: ' + rel);
          }
          const path = source + '/' + rel;
          mkdirTree(m.FS, path.slice(0, path.lastIndexOf('/')));
          m.FS.writeFile(path, new Uint8Array(await wanted[i].file.arrayBuffer()));
          if ((i & 31) === 31) {
            showProgress('Reading your game files\u2026 ' + (i + 1) + ' of ' + wanted.length);
            await new Promise((resolve) => setTimeout(resolve, 0));
          }
        }
        bridgeComplete(request.id, true, source);
      } catch (err) { bridgeComplete(request.id, false, err && err.message ? err.message : String(err)); }
      close();
    });
    folderOverlay = {
      show: function (request) {
        overlayRequest = request;
        lead.textContent = RETAIL_FOLDER_LEAD;
        button.textContent = 'Choose your game folder';
        hints.replaceChildren(...RETAIL_FOLDER_HINTS.map(function (line) {
          const paragraph = document.createElement('p');
          paragraph.className = 'hint';
          paragraph.textContent = line;
          return paragraph;
        }));
        close();
        overlay.hidden = false;
        button.focus();
        // Straight from a click in the launcher the browser still allows
        // opening the picker, which saves the player a second click.
        if (navigator.userActivation && navigator.userActivation.isActive) input.click();
      },
    };
    return folderOverlay;
  }

  function exposeBridge(m) {
    m.hp2Web = {
      dataSourceStatus: function (requestId) {
        const id = requestId === undefined ? nextId() : requestId;
        Promise.resolve().then(async function () {
          const opfs = await navigator.storage.getDirectory();
          await migrateLegacyPrototype(opfs);
          const result = {
            retail: await hasMarker(opfs, 'retail'),
            prototype: await hasMarker(opfs, 'prototype'),
          };
          bridgeComplete(id, true, JSON.stringify(result));
        }).catch((err) => bridgeComplete(id, false, String(err)));
      },
      pickFolder: function (requestId, kind) {
        if (typeof requestId === 'string') { kind = requestId; requestId = nextId(); }
        const id = requestId === undefined ? nextId() : requestId;
        // Players only ever hand over the game folder: every import is copied
        // into this browser's storage, so nothing else may be picked here.
        if (kind !== 'retail') {
          bridgeComplete(id, false, 'Unknown data source: ' + kind);
          return;
        }
        makeFolderOverlay().show({ id: id });
      },
      persistImport: function (requestId, sourcePath, destinationKind) {
        if (typeof requestId === 'string') {
          destinationKind = sourcePath;
          sourcePath = requestId;
          requestId = nextId();
        }
        const id = requestId === undefined ? nextId() : requestId;
        Promise.resolve().then(async function () {
          if (!currentModule || (destinationKind !== 'retail' && destinationKind !== 'prototype')) {
            throw new Error('Invalid import destination.');
          }
          await copyFsTreeToOpfs(currentModule, sourcePath, destinationKind);
          removeFsTree(currentModule.FS, sourcePath);
          removeFsTree(currentModule.FS, '/hp2import/source');
          bridgeComplete(id, true, '');
        }).catch((err) => bridgeComplete(id, false, String(err)));
      },
      stageDataSource: function (requestId, kind) {
        if (typeof requestId === 'string') { kind = requestId; requestId = nextId(); }
        const id = requestId === undefined ? nextId() : requestId;
        Promise.resolve().then(async function () {
          if (kind !== 'retail' && kind !== 'prototype') throw new Error('Unknown data source: ' + kind);
          const copied = await copyOpfsTreeToFs(kind, currentModule);
          if (!copied) throw new Error('The ' + kind + ' data source is empty.');
          bridgeComplete(id, true, DATA_ROOT + '/' + kind);
        }).catch((err) => bridgeComplete(id, false, String(err)));
      },
      syncUser: function (requestId) {
        const shouldReport = requestId !== undefined;
        const id = shouldReport ? requestId : 0;
        if (!currentModule || !currentModule.FS) {
          if (shouldReport) bridgeComplete(id, false, 'User filesystem is unavailable.');
          else logToBoth('IDBFS sync failed: user filesystem is unavailable.');
          return;
        }
        currentModule.FS.syncfs(false, function (err) {
          if (shouldReport) bridgeComplete(id, !err, err ? String(err) : '');
          else if (err) logToBoth('IDBFS sync failed: ' + err);
        });
      },
    };
  }

  function syncFilesystem() {
    if (!currentModule || !currentModule.FS) return;
    currentModule.FS.syncfs(false, (err) => { if (err) logToBoth('IDBFS sync failed: ' + err); });
  }

  function startPersistence(instance) {
    currentModule = instance;
    setInterval(function () {
      if (syncInFlight) return;
      syncInFlight = true;
      instance.FS.syncfs(false, function (err) {
        syncInFlight = false;
        if (err) logToBoth('IDBFS sync failed: ' + err);
      });
    }, 10000);
    document.addEventListener('visibilitychange', function () {
      if (document.visibilityState === 'hidden') syncFilesystem();
    });
  }

  function resumeAudioContexts() {
    const al = currentModule && currentModule.AL;
    if (!al) return;
    const contexts = [];
    if (al.currentCtx) contexts.push(al.currentCtx);
    for (const context of Object.values(al.contexts || {})) {
      if (contexts.indexOf(context) < 0) contexts.push(context);
    }
    for (const context of contexts) {
      if (context && context.audioCtx && context.audioCtx.state === 'suspended') {
        context.audioCtx.resume();
      }
    }
  }

  function finish(text) {
    syncFilesystem();
    document.body.classList.remove('running');
    setStatus(text);
    reportSmoke('failed', text);
  }

  async function entriesFromDevServer() {
    const response = await fetch('/dev-data/manifest.json', { cache: 'no-store' });
    if (!response.ok) throw new Error('dev data manifest unavailable (HTTP ' + response.status + ')');
    return (await response.json()).map((path) => ({
      path: path,
      read: async function () {
        const encoded = path.split('/').map(encodeURIComponent).join('/');
        const result = await fetch('/dev-data/' + encoded, { cache: 'no-store' });
        if (!result.ok) throw new Error('dev data fetch failed for ' + path + ' (HTTP ' + result.status + ')');
        return result.arrayBuffer();
      },
    }));
  }

  async function requestDirectBoot() {
    if (!params.has('map')) return;
    const map = params.get('map');
    if (!map) return;
    const opfs = await navigator.storage.getDirectory();
    const kind = await hasMarker(opfs, 'prototype') ? 'prototype' : 'retail';
    if (!await hasMarker(opfs, kind)) {
      throw new Error('No imported retail or prototype data is available for direct map boot.');
    }
    await copyOpfsTreeToFs(kind, currentModule);
    currentModule.FS.chdir(DATA_ROOT + '/' + kind + '/System');
    started = true;
    document.body.classList.add('running');
    setStatus('Starting ' + map + '…');
    resumeAudioContexts();
    canvas.addEventListener('pointerdown', resumeAudioContexts);
    canvas.getContext('webgl2', { preserveDrawingBuffer: true, alpha: false, depth: true, stencil: false, antialias: false });
    const args = [map, '-datadir=' + DATA_ROOT + '/' + kind, '-LOG'];
    logToBoth('Launching: ' + args.join(' '));
    currentModule.callMain(args);
  }

  // The browser keeps its view controls, but all data selection and launch
  // policy now belongs to the in-engine SDL/ImGui launcher.
  function syncFullscreenButton() {
    fullscreenToggle.textContent = document.fullscreenElement ? '⤡' : '⤢';
    fullscreenToggle.title = document.fullscreenElement ? 'Leave full screen (F)' : 'Full screen (F)';
  }
  fullscreenToggle.addEventListener('click', async function () {
    try {
      if (document.fullscreenElement) await document.exitFullscreen();
      else await document.documentElement.requestFullscreen();
    } catch (err) { logToBoth('Full screen was refused by the browser: ' + err); }
  });
  document.addEventListener('fullscreenchange', syncFullscreenButton);
  syncFullscreenButton();
  document.addEventListener('keydown', function (event) {
    if (event.key === 'f' || event.key === 'F') fullscreenToggle.click();
  });
  canvas.addEventListener('pointerdown', resumeAudioContexts);
  canvas.addEventListener('keydown', resumeAudioContexts);
  logToggle.addEventListener('click', function () { logEl.classList.toggle('open'); });

  // Save backup / Restore backup: the user tree under USER_ROOT (saves,
  // settings, Launcher.ini) as a store-only zip, so progress can outlive
  // cleared site data or move to another computer.
  const BACKUP_MANIFEST = 'hp2-saves.json';
  const GAME_LOADING = 'The game is still loading. Try again in a moment.';
  backupSave.addEventListener('click', function () {
    if (!currentModule) { setStatus(GAME_LOADING); return; }
    try {
      const entries = readFsTree(currentModule.FS, USER_ROOT,
        (name) => !name.toLowerCase().endsWith('.log'));
      const manifest = { format: 1, exportedAt: new Date().toISOString(), origin: location.host };
      entries.push({ path: BACKUP_MANIFEST, data: new TextEncoder().encode(JSON.stringify(manifest)) });
      const now = new Date();
      const day = now.getFullYear() + '-' + String(now.getMonth() + 1).padStart(2, '0') + '-'
        + String(now.getDate()).padStart(2, '0');
      const url = URL.createObjectURL(buildStoredZip(entries));
      const link = document.createElement('a');
      link.href = url;
      link.download = 'hp2-saves-' + day + '.zip';
      document.body.appendChild(link);
      link.click();
      link.remove();
      setTimeout(function () { URL.revokeObjectURL(url); }, 60000);
      setStatus('Backup saved to your Downloads folder. Keep it somewhere safe.');
    } catch (err) { setStatus('Saving the backup failed: ' + err); }
  });
  backupRestore.addEventListener('click', function () {
    if (!currentModule) { setStatus(GAME_LOADING); return; }
    backupInput.click();
  });
  backupInput.addEventListener('change', async function () {
    const file = backupInput.files && backupInput.files[0];
    if (!file) return;
    try {
      if (!currentModule) throw new Error(GAME_LOADING);
      const entries = readStoredZip(await file.arrayBuffer());
      const manifest = entries.find((entry) => entry.path === BACKUP_MANIFEST);
      let format = null;
      try { format = manifest && JSON.parse(new TextDecoder().decode(manifest.data)).format; }
      catch (err) { format = null; }
      if (format !== 1) throw new Error(NOT_A_BACKUP);
      for (const entry of entries) {
        if (entry.path.indexOf('\\') >= 0 || entry.path.startsWith('/')
          || entry.path.split('/').some((part) => !part || part === '.' || part === '..')) {
          throw new Error(NOT_A_BACKUP);
        }
      }
      if (!confirm('Restoring replaces the progress stored in this browser with the backup. Continue?')) return;
      const fs = currentModule.FS;
      // USER_ROOT itself is the IDBFS mount point, so only its children go.
      for (const name of fs.readdir(USER_ROOT)) {
        if (name !== '.' && name !== '..') removeFsTree(fs, USER_ROOT + '/' + name);
      }
      for (const entry of entries) {
        if (entry.path === BACKUP_MANIFEST) continue;
        const path = USER_ROOT + '/' + entry.path;
        mkdirTree(fs, path.slice(0, path.lastIndexOf('/')));
        fs.writeFile(path, entry.data);
      }
      await new Promise(function (resolve, reject) {
        fs.syncfs(false, function (err) { if (err) reject(err); else resolve(); });
      });
      location.reload();
    } catch (err) {
      setStatus('Restoring failed: ' + (err && err.message ? err.message : err));
    } finally {
      backupInput.value = '';
    }
  });

  const FONT_DIR = '/hp2fonts';
  const FONT_FILES = ['LiberationSans-Regular.ttf', 'LiberationSans-Bold.ttf',
    'LiberationMono-Regular.ttf', 'NotoSansSC-Regular.otf'];
  async function stageFonts(m) {
    mkdirTree(m.FS, FONT_DIR);
    for (const name of FONT_FILES) {
      try {
        const response = await fetch('fonts/' + name, { cache: 'force-cache' });
        if (!response.ok) throw new Error('HTTP ' + response.status);
        m.FS.writeFile(FONT_DIR + '/' + name, new Uint8Array(await response.arrayBuffer()));
      } catch (err) { logToBoth('Font not staged: ' + name + ' (' + err + ')'); }
    }
  }

  function createModule() {
    return createHP2Module({
      canvas: canvas,
      thisProgram: 'HarryPotter2',
      noExitRuntime: true,
      print: logToBoth,
      printErr: logToBoth,
      onHP2Exit: (code) => finish('Game exited (code ' + code + ').'),
      onExit: (code) => finish('Game exited (code ' + code + ').'),
      onAbort: (reason) => finish(reason ? 'Game crashed: ' + reason : 'Game exited.'),
      preRun: [function (m) {
        exposeBridge(m);
        m.addRunDependency('hp2-font');
        stageFonts(m).then(function () { m.removeRunDependency('hp2-font'); },
          function (err) {
            logToBoth('Font staging failed: ' + err);
            m.removeRunDependency('hp2-font');
          });
        m.ENV.XDG_DATA_HOME = HOME_PREFIX;
        mkdirTree(m.FS, USER_ROOT);
        m.FS.mount(m.FS.filesystems.IDBFS, {}, USER_ROOT);
        m.addRunDependency('hp2-idbfs');
        m.FS.syncfs(true, function (err) {
          if (err) logToBoth('IDBFS load failed: ' + err);
          m.removeRunDependency('hp2-idbfs');
        });
      }],
    });
  }

  (async function init() {
    if (!self.crossOriginIsolated) {
      setStatus(COOP_STATUS);
      reportSmoke('failed', COOP_STATUS);
      return;
    }
    if (!WebAssembly.validate(MEMORY64_PROBE)) {
      setStatus(UNSUPPORTED_BROWSER);
      reportSmoke('failed', 'no wasm memory64');
      return;
    }
    if (!navigator.storage || !('getDirectory' in navigator.storage)) {
      setStatus('This browser has no private file storage, so the game cannot be installed here.');
      reportSmoke('failed', 'no origin-private file system (OPFS)');
      return;
    }
    try {
      // Keep the most recently presented canvas pixels available to the
      // browser launcher smoke screenshot.
      if (!canvas.getContext('webgl2', {
        preserveDrawingBuffer: true, alpha: false, depth: true, stencil: false, antialias: false,
      })) throw new Error('WebGL2 is unavailable.');
      const instance = await createModule();
      currentModule = instance;
      exposeBridge(instance);
      startPersistence(instance);
      const opfs = await navigator.storage.getDirectory();
      await migrateLegacyPrototype(opfs);
      if (devMode && !await hasMarker(opfs, 'prototype')) {
        try { await importEntries(await entriesFromDevServer()); }
        catch (err) { setStatus('Dev data import failed: ' + err); }
      }
      if (smokeMode === 'shell') reportSmoke('ready', 'module instantiated');
      if (params.has('map')) {
        await requestDirectBoot();
      } else {
        setStatus('Starting launcher…');
        currentModule.callMain(['-LOG']);
      }
    } catch (err) {
      setStatus('Loading the game failed: ' + err);
      logToBoth('Web startup failed: ' + err);
      reportSmoke('failed', 'module instantiation failed: ' + err);
    }
  })();
})();
