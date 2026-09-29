// Browser shell for the Harry Potter 2 WebAssembly build (milestone 1).
//
// Data flow: the player imports their own data folder once, the page copies it
// into OPFS, and each launch copies it into the MEMFS root the engine reads.
// The user root is mounted from IDBFS so saves and ini files survive a reload.
//
// Every claim this file supports is under the data-prototype profile.
'use strict';

(function () {
  const OPFS_DIR = 'hp2-data';
  const MARKER = '.import-complete.json';
  const DATA_ROOT = '/hp2data';
  const HOME_PREFIX = '/hp2home';
  // Must match ResolveDataHomePrefix + UserSuffix in Launch/Src/HP2Paths.cpp.
  const USER_ROOT = '/hp2home/harry-potter-2/User';
  const COOP_STATUS =
    'This page must be served with COOP/COEP headers (python3 Build/serve_web.py).';
  // The engine logs a lot during startup, so the page keeps a generous window
  // rather than truncating mid-boot.
  const LOG_LINES = 20000;

  const canvas = document.getElementById('canvas');
  const setupEl = document.getElementById('setup');
  const chooserEl = document.getElementById('chooser');
  const importInput = document.getElementById('import');
  const chooseFolderButton = document.getElementById('choose-folder');
  const setupStatus = document.getElementById('setup-status');
  const saveList = document.getElementById('save-list');
  const noSaves = document.getElementById('no-saves');
  const newGameButton = document.getElementById('new-game');
  const continueButton = document.getElementById('continue');
  const statusEl = document.getElementById('status');
  const logEl = document.getElementById('log');
  const logToggle = document.getElementById('log-toggle');
  const params = new URLSearchParams(location.search);
  // ?devdata additionally offers a button that imports from the dev server,
  // for local testing against a served data root. Never shown otherwise.
  const devMode = params.has('devdata');

  // The module is instantiated at page init (see init), so FS, the IDBFS
  // user root and the save list are all available before any launch.
  // smoke=shell only reports that the module came up; smoke=boot additionally
  // imports dev data, launches, and counts main-loop frames.
  const smokeMode = ['shell', 'boot'].indexOf(params.get('smoke')) >= 0
    ? params.get('smoke')
    : null;

  let syncInFlight = false;
  let started = false;
  let moduleReady = false;
  let dataImported = false;
  let finished = false;
  let smokeReported = false;
  let currentModule = null;
  let saves = [];
  let selectedSave = -1;

  function setStatus(text) {
    statusEl.textContent = text;
  }

  function logLine(text) {
    const atBottom = logEl.scrollTop + logEl.clientHeight >= logEl.scrollHeight - 4;
    logEl.textContent += text + '\n';
    let lines = logEl.textContent.split('\n');
    if (lines.length > LOG_LINES) {
      lines = lines.slice(lines.length - LOG_LINES);
      logEl.textContent = lines.join('\n');
    }
    if (atBottom) {
      logEl.scrollTop = logEl.scrollHeight;
    }
  }

  // Mirror the module's stdout/stderr into the page and the devtools console.
  function logToBoth(text) {
    logLine(text);
    console.log(text);
  }

  // Headless smoke hook: Build/web_smoke.py subclasses the dev server to
  // accept one POST per run, so the page reports exactly one terminal stage
  // together with the tail of the engine log. Non-smoke pages never call it.
  function reportSmoke(stage, detail, extra) {
    if (smokeMode === null || smokeReported) {
      return;
    }
    smokeReported = true;
    const lines = logEl.textContent.split('\n');
    const payload = {
      mode: smokeMode,
      stage: stage,
      detail: detail,
      frames: 0,
      log: lines.slice(Math.max(0, lines.length - 400)).join('\n'),
    };
    if (extra) {
      Object.assign(payload, extra);
    }
    fetch('/__hp2_smoke', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    }).catch(function (err) {
      console.error('smoke report failed: ' + err);
    });
  }

  // The page's own controls. Continue additionally needs a save on disk, so
  // the shell can offer exactly what the native launcher offers.
  function updateButtons() {
    const canLaunch = moduleReady && dataImported && !started;
    newGameButton.disabled = !canLaunch;
    // Native launcher parity: Continue needs a save *selected*, not merely
    // one existing (shell.qml gates it on selectedSaveIndex >= 0).
    continueButton.disabled = !canLaunch || selectedSave < 0;
  }

  function formatBytes(bytes) {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
    return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
  }

  // Mirrors CANONICAL_ROOTS in Build/prepare_retail_data.py, plus the
  // System/Cutscenes output the same script produces.
  //
  // music is deliberately the one lowercase root. PlayMusic opens
  // "..\music\<song>.ogg" from the System working directory
  // (ALAudioSubsystem.cpp), and FFileStream::CreateFileLocked reaches it
  // through appFopen, a bare fopen with no case-insensitive fallback -- the
  // only caller of appFopen. Every other root is read through FFileManagerUnix,
  // whose RecoverPath resolves the wrong case, which is why Maps and Textures
  // load from the capitalized directories today. Shipping the music tree under
  // one lowercase copy avoids a second ~59 MB duplicate in MEMFS.
  const CANONICAL_ROOTS = {
    system: 'System',
    maps: 'Maps',
    textures: 'Textures',
    sounds: 'Sounds',
    music: 'music',
    help: 'Help',
  };
  const SUFFIX = 'System/Default.ini';

  // Returns the path to write inside the OPFS data root, or null to skip.
  function normalizeDataPath(rel) {
    const parts = String(rel).split('/').filter((p) => p.length > 0);
    if (parts.length === 0) return null;
    if (parts.length === 1) {
      // Overlay manifests travel next to the data they describe.
      return parts[0] === 'overlay-manifest.json' || parts[0] === 'overlay-checksums.txt'
        ? parts[0]
        : null;
    }
    const root = parts[0].toLowerCase();
    if (parts.length === 2) {
      const canonical = CANONICAL_ROOTS[root];
      // Cutscene subtitles are not game data and only bloat the import.
      if (!canonical || parts[1].toLowerCase().endsWith('.scc')) return null;
      return canonical + '/' + parts[1];
    }
    if (root === 'system' && parts[1].toLowerCase() === 'cutscenes') {
      return ['System', 'Cutscenes'].concat(parts.slice(2)).join('/');
    }
    return null;
  }
  // The imported tree can be rooted anywhere above System/Default.ini: a
  // folder picker adds a top-level folder name, the dev route does not. The
  // shortest such prefix is the data root, and the remainder is each entry's
  // path relative to it.
  function findDataRoot(paths) {
    const normalized = paths.map((p) => p.replace(/\\/g, '/'));
    const suffix = SUFFIX.toLowerCase();
    const matches = normalized.filter(
      (p) => p.toLowerCase() === suffix || p.toLowerCase().endsWith('/' + suffix));
    if (matches.length === 0) {
      throw new Error('No System/Default.ini found in the selected folder.');
    }
    matches.sort((a, b) => a.length - b.length);
    const shortest = matches[0];
    const root = shortest.length === suffix.length
      ? ''
      : shortest.slice(0, shortest.length - (suffix.length + 1));
    return {
      root: root,
      relativize: (p) => (root && p.startsWith(root + '/') ? p.slice(root.length + 1) : p),
    };
  }

  async function writeToOpfs(rootDir, relative, data) {
    const parts = relative.split('/');
    let dir = rootDir;
    for (let i = 0; i < parts.length - 1; ++i) {
      dir = await dir.getDirectoryHandle(parts[i], { create: true });
    }
    const handle = await dir.getFileHandle(parts[parts.length - 1], { create: true });
    const writable = await handle.createWritable();
    await writable.write(data);
    await writable.close();
  }

  async function importEntries(entries) {
    const paths = entries.map((e) => e.path);
    const dataRoot = findDataRoot(paths);
    const opfsRoot = await navigator.storage.getDirectory();
    try {
      await opfsRoot.removeEntry(OPFS_DIR, { recursive: true });
    } catch (err) {
      if (err && err.name !== 'NotFoundError') throw err;
    }
    const dataDir = await opfsRoot.getDirectoryHandle(OPFS_DIR, { create: true });

    let written = 0;
    let bytes = 0;
    const total = entries.length;
    for (const entry of entries) {
      const target = normalizeDataPath(dataRoot.relativize(entry.path));
      if (target === null) {
        continue;
      }
      let data;
      try {
        data = await entry.read();
      } catch (err) {
        throw new Error(entry.path + ': ' + err);
      }
      await writeToOpfs(dataDir, target, data);
      written += 1;
      bytes += data.byteLength;
      setSetupStatus('Importing ' + written + '/' + total + ' (' + formatBytes(bytes) + ')');
    }

    // Written last: its presence is the only signal that an import completed.
    const marker = {
      files: written,
      bytes: bytes,
      importedAt: new Date().toISOString(),
    };
    await writeToOpfs(dataDir, MARKER, new Blob([JSON.stringify(marker)], {
      type: 'application/json',
    }));
    dataImported = true;
    showChooser();
    setStatus('Ready. ' + written + ' files imported.');
  }

  async function hasMarker() {
    try {
      const opfsRoot = await navigator.storage.getDirectory();
      const dataDir = await opfsRoot.getDirectoryHandle(OPFS_DIR);
      await dataDir.getFileHandle(MARKER);
      return true;
    } catch (err) {
      return false;
    }
  }

  async function entriesFromFolderInput(fileList) {
    const files = Array.from(fileList).filter((f) => f.size > 0 || f.type.length > 0);
    return files.map((f) => ({
      path: f.webkitRelativePath || f.name,
      read: () => f.arrayBuffer(),
    }));
  }

  async function entriesFromDevServer() {
    setStatus('Reading dev data manifest…');
    const response = await fetch('/dev-data/manifest.json', { cache: 'no-store' });
    if (!response.ok) {
      throw new Error('dev data manifest unavailable (HTTP ' + response.status + ')');
    }
    const paths = await response.json();
    return paths.map((path) => ({
      path: path,
      read: async () => {
        const encoded = path.split('/').map(encodeURIComponent).join('/');
        const fileResponse = await fetch('/dev-data/' + encoded, { cache: 'no-store' });
        if (!fileResponse.ok) {
          throw new Error('dev data fetch failed for ' + path + ' (HTTP ' + fileResponse.status + ')');
        }
        return fileResponse.arrayBuffer();
      },
    }));
  }

  async function listOpfsDataFiles(dir, prefix) {
    const results = [];
    for await (const [name, handle] of dir.entries()) {
      if (handle.kind === 'directory') {
        const child = await dir.getDirectoryHandle(name);
        const nested = await listOpfsDataFiles(child, prefix + name + '/');
        results.push(...nested);
      } else if (prefix + name !== MARKER) {
        results.push({ handle: handle, path: prefix + name });
      }
    }
    return results;
  }

  function mkdirTree(fs, path) {
    const parts = path.split('/').filter((p) => p.length > 0);
    let current = '';
    for (const part of parts) {
      current += '/' + part;
      try {
        fs.mkdir(current);
      } catch (err) {
        // Already present.
      }
    }
  }

  // Walks the imported OPFS tree and mirrors it into the module's MEMFS. The
  // walk yields to the event loop periodically so the page keeps painting
  // while a few hundred megabytes are copied.
  // Writes dev-server entries straight into the module's filesystem, applying
  // the same path normalisation the import uses. Used by the boot smoke so a
  // test run does not pay for an OPFS round trip it will never read back.
  async function copyDevServerDataIntoFs(entries) {
    const dataRoot = findDataRoot(entries.map(function (entry) { return entry.path; }));
    const m = window.hp2Module;
    mkdirTree(m.FS, DATA_ROOT);
    let copied = 0;
    let bytes = 0;
    for (const entry of entries) {
      const target = normalizeDataPath(dataRoot.relativize(entry.path));
      if (target === null) {
        continue;
      }
      const buffer = await entry.read();
      const path = DATA_ROOT + '/' + target;
      mkdirTree(m.FS, path.slice(0, path.lastIndexOf('/')));
      m.FS.writeFile(path, new Uint8Array(buffer));
      ++copied;
      bytes += buffer.byteLength;
      if ((copied & 63) === 0) {
        setStatus('Loading ' + copied + '/' + entries.length +
          ' (' + formatBytes(bytes) + ')');
        await new Promise(function (resolve) { setTimeout(resolve, 0); });
      }
    }
    logToBoth('Loaded ' + copied + ' data files (' + formatBytes(bytes) +
      ') into ' + DATA_ROOT);
    return copied;
  }

  async function copyOpfsDataIntoFs(m, dataDir) {
    const files = await listOpfsDataFiles(dataDir, '');
    mkdirTree(m.FS, DATA_ROOT);
    let copied = 0;
    for (const file of files) {
      const buffer = await file.handle.getFile().then(function (f) { return f.arrayBuffer(); });
      const target = DATA_ROOT + '/' + file.path;
      mkdirTree(m.FS, target.slice(0, target.lastIndexOf('/')));
      m.FS.writeFile(target, new Uint8Array(buffer));
      ++copied;
      if ((copied & 63) === 0) {
        await new Promise(function (resolve) { setTimeout(resolve, 0); });
      }
    }
    return copied;
  }

  // Save discovery mirrors DiscoverSaves/SavesIn/SaveName/SlotName in
  // SDLLaunch/Src/HP2LauncherStore.cpp, and the launch arguments mirror
  // LaunchArguments in SDLLaunch/Src/HP2LaunchPolicy.cpp. The engine does
  // the same parsing natively; the browser shell only has to agree on which
  // files are saves and what to pass on the command line.
  //
  // C++ canonicalises the digits ("07" is not index 7), so a name whose
  // digits differ from their decimal form is not a save at all.
  function canonicalIndex(digits) {
    const value = parseInt(digits, 10);
    return String(value) === digits ? value : null;
  }

  function saveFilesIn(fs, directory, slot, slotted, out) {
    let names;
    try {
      names = fs.readdir(directory);
    } catch (err) {
      return;
    }
    for (const name of names) {
      const match = /^save(\d+)\.usa$/i.exec(name);
      if (!match) {
        continue;
      }
      const index = canonicalIndex(match[1]);
      if (index === null) {
        continue;
      }
      const path = directory + '/' + name;
      let stat;
      try {
        stat = fs.stat(path);
      } catch (err) {
        continue;
      }
      // lstat + S_ISREG + size > 0 in the native rule; a zero-length or
      // non-regular entry is not a usable save.
      if (!fs.isDir(stat.mode) && stat.size > 0) {
        out.push({
          slot: slot,
          index: index,
          slotted: slotted,
          mtime: stat.mtime.getTime(),
          path: path,
          label: slotted
            ? 'Slot ' + slot + ' - Save ' + index
            : 'Save ' + index,
        });
      }
    }
  }

  function listSaves(fs) {
    const root = USER_ROOT + '/Save';
    const found = [];
    let names;
    try {
      names = fs.readdir(root);
    } catch (err) {
      // A missing save directory is the first-run state, not an error.
      return found;
    }
    saveFilesIn(fs, root, -1, false, found);
    for (const name of names) {
      const match = /^slot(\d+)$/i.exec(name);
      if (!match) {
        continue;
      }
      const slot = canonicalIndex(match[1]);
      if (slot === null) {
        continue;
      }
      saveFilesIn(fs, root + '/' + name, slot, true, found);
    }
    found.sort(function (a, b) {
      if (a.slotted !== b.slotted) {
        return a.slotted ? 1 : -1;
      }
      if (!a.slotted && a.index !== b.index) {
        return a.index - b.index;
      }
      if (a.slotted && a.slot !== b.slot) {
        return a.slot - b.slot;
      }
      if (a.index !== b.index) {
        return a.index - b.index;
      }
      return a.path < b.path ? -1 : (a.path > b.path ? 1 : 0);
    });
    return found;
  }

  // Fills the save list. The most recently modified save is preselected,
  // matching the native launcher's default, and each row shows the same
  // name-and-timestamp pair the QML delegate renders.
  function refreshSaves() {
    if (!currentModule || !currentModule.FS) {
      return;
    }
    saves = listSaves(currentModule.FS);
    saveList.textContent = '';
    saves.forEach(function (save, position) {
      const row = document.createElement('li');
      row.className = 'save';
      row.dataset.position = String(position);
      const name = document.createElement('div');
      name.className = 'name';
      name.textContent = save.label;
      const meta = document.createElement('div');
      meta.className = 'meta';
      meta.textContent = save.mtime ? new Date(save.mtime).toLocaleString() : '';
      row.appendChild(name);
      row.appendChild(meta);
      row.addEventListener('click', function () {
        selectSave(position);
      });
      saveList.appendChild(row);
    });
    noSaves.hidden = saves.length > 0;
    // Preselect the newest, so Continue is useful without a tap.
    selectSave(saves.length > 0 ? newestSavePosition() : -1);
  }

  function newestSavePosition() {
    let newest = 0;
    for (let i = 1; i < saves.length; ++i) {
      if (saves[i].mtime > saves[newest].mtime) {
        newest = i;
      }
    }
    return newest;
  }

  function selectSave(position) {
    selectedSave = position;
    Array.prototype.forEach.call(saveList.children, function (row) {
      row.classList.toggle('selected', Number(row.dataset.position) === position);
    });
    updateButtons();
  }

  // Runs main with the launcher's arguments. The module already exists (it is
  // instantiated at page init), so this only copies the imported data into
  // MEMFS and hands control to the engine.
  async function launch(args) {
    if (started) {
      return;
    }
    started = true;
    updateButtons();
    try {
      const opfsRoot = await navigator.storage.getDirectory();
      // The marker and the data live in the same OPFS directory, but a
      // profile can carry one without the other (an import that wrote the
      // marker last, or a half-cleared profile). Missing data is the setup
      // step, not a launch failure, so send the player back to it.
      // Already in place: the boot smoke puts the data in the module's own
      // filesystem, and a second copy out of OPFS would be pure waste.
      if (dataRootReady()) {
        logToBoth('Game data already present in ' + DATA_ROOT);
      } else {
      let dataDir;
      try {
        dataDir = await opfsRoot.getDirectoryHandle(OPFS_DIR);
      } catch (err) {
        dataImported = false;
        showSetup();
        setSetupStatus('The imported game data is missing. Choose your folder again.');
        reportSmoke('failed', 'imported game data is missing');
        started = false;
        updateButtons();
        return;
      }
      const copied = await copyOpfsDataIntoFs(window.hp2Module, dataDir);
      if (copied === 0) {
        dataImported = false;
        showSetup();
        setSetupStatus('The imported game data is empty. Choose your folder again.');
        reportSmoke('failed', 'imported game data is empty');
        started = false;
        updateButtons();
        return;
      }
      logToBoth('Loaded ' + copied + ' data files into ' + DATA_ROOT);
      }
      // Seed the user ini from the data root, which now exists either way: the
      // engine copies Default.ini and DefUser.ini into the user root at startup,
      // and doing it here means the settings panel edits the engine's real
      // defaults rather than a file it would otherwise have to invent.
      seedUserIniFromData();
      // The seeded ini is what the engine is about to read, so re-resolve
      // against it and write the outcome: on a first run that is what puts
      // the launcher's defaults in front of the engine instead of the data
      // root's 2002 values, and a key the player has set is written over the
      // prototype's. Choosing a value earlier in the panel lands here too.
      loadSettings();
      applySettingsToControls();
      await saveSettings();
      // The engine addresses packages relative to the process working
      // directory, which is the data root's System directory: it logs
      // "..\\Maps\\..." and "..\\music\\..." style paths (see
      // Launch/Src/HP2Paths.cpp appSetBaseDir). That directory only exists
      // once the copy above has finished, so the switch happens here.
      try {
        window.hp2Module.FS.chdir(DATA_ROOT + '/System');
        logToBoth('Working directory is now ' + window.hp2Module.FS.cwd());
      } catch (err) {
        logToBoth('chdir(' + DATA_ROOT + '/System) failed: ' +
          (err && err.message ? err.message : String(err)));
      }
    } catch (err) {
      // main is never called on a failed copy: the engine would boot against
      // a half-populated data root.
      started = false;
      setStatus('Loading game data failed: ' + err);
      reportSmoke('failed', 'Loading game data failed: ' + err);
      updateButtons();
      return;
    }

    // SDL creates the WebGL context itself; preserveDrawingBuffer is not
    // exposed through the module options, so the context is requested up front
    // and SDL keeps the one it gets.
    canvas.getContext('webgl2', {
      preserveDrawingBuffer: true,
      alpha: false,
      depth: true,
      stencil: false,
      antialias: false,
    });

    // The launcher is the primary surface; the game replaces it only once
    // the engine is actually running.
    document.body.classList.add('running');
    setStatus('Running…');
    resumeAudioContexts();
    canvas.addEventListener('pointerdown', resumeAudioContexts);
    canvas.addEventListener('keydown', resumeAudioContexts);
    canvas.focus();
    const full = args.concat(['-datadir=' + DATA_ROOT, '-NOFRONTEND', '-LOG']);
    logToBoth('Launching: ' + full.join(' '));
    window.hp2Module.callMain(full);
  }

  function newGameArgs() {
    // ?map= is a development override for pointing the browser at one map.
    return [params.get('map') || 'PrivetDr.unr'];
  }

  function continueArgs() {
    const save = saves[selectedSave];
    if (!save) {
      return null;
    }
    const args = ['Startup.unr', '-LOAD=' + save.index];
    if (save.slotted) {
      args.push('-SAVESLOT=' + save.slot);
    }
    return args;
  }

  // Every exit path reports through here: FinishProcess from the engine, the
  // runtime's own exit, and a runtime abort. The IDBFS flush must happen
  // before the player reloads, and the page must not sit on "Running…"
  // forever when the engine dies during init.
  function finish(text) {
    if (finished) {
      return;
    }
    finished = true;
    // The runtime is still alive here (EXIT_RUNTIME=0), so the IDBFS
    // contents can be flushed before the user reloads the page.
    syncFilesystem(false);
    // Back to the launcher rather than a dead page: the player can pick
    // another save or start over without reloading.
    document.body.classList.remove('running');
    setStatus(text + ' Reload the page to play again.');
    reportSmoke('failed', text);
  }

  // Chrome starts an AudioContext suspended unless it is created or resumed
  // inside a user gesture, and the engine opens its device from a worker, so
  // every context that ends up suspended is resumed from the next click or key
  // press on the canvas.
  function resumeAudioContexts() {
    const al = window.hp2Module && window.hp2Module.AL;
    if (!al) {
      return;
    }
    const contexts = [];
    if (al.currentCtx) {
      contexts.push(al.currentCtx);
    }
    for (const ctx of Object.values(al.contexts || {})) {
      if (contexts.indexOf(ctx) === -1) {
        contexts.push(ctx);
      }
    }
    for (const ctx of contexts) {
      if (ctx && ctx.audioCtx && ctx.audioCtx.state === 'suspended') {
        ctx.audioCtx.resume();
      }
    }
  }

  function syncFilesystem(populate) {
    // populate=true would overwrite MEMFS from IndexedDB, discarding anything
    // the run has written; it is only correct for the initial load, which the
    // preRun hook does directly. Callers here always want a flush (false).
    if (!currentModule || !currentModule.FS) {
      return;
    }
    try {
      currentModule.FS.syncfs(populate, function (err) {
        if (err) {
          logToBoth('IDBFS sync failed: ' + err);
        }
      });
    } catch (err) {
      logToBoth('IDBFS sync unavailable: ' + err);
    }
  }

  function startPersistence(instance) {
    currentModule = instance;
    setInterval(function () {
      if (syncInFlight || !currentModule) {
        return;
      }
      syncInFlight = true;
      currentModule.FS.syncfs(false, function (err) {
        syncInFlight = false;
        if (err) {
          logToBoth('IDBFS sync failed: ' + err);
        }
      });
    }, 10000);
    document.addEventListener('visibilitychange', function () {
      if (document.visibilityState === 'hidden') {
        syncFilesystem(false);
      }
    });
  }

  // The first-run step is one button, not a file input the player has to
  // understand: the label opens the folder picker and the file input stays
  // hidden behind it.
  chooseFolderButton.addEventListener('click', function () {
    importInput.click();
  });

  importInput.addEventListener('change', async function () {
    if (importInput.files.length === 0) {
      return;
    }
    dataImported = false;
    showSetup();
    setSetupStatus('Copying your game files into this browser…');
    try {
      await importEntries(await entriesFromFolderInput(importInput.files));
    } catch (err) {
      setSetupStatus('Import failed: ' + err);
    }
  });

  if (devMode) {
    const devButton = document.createElement('button');
    devButton.textContent = 'Import from dev server';
    devButton.addEventListener('click', async function () {
      dataImported = false;
      showSetup();
      setSetupStatus('Reading dev data…');
      try {
        await importEntries(await entriesFromDevServer());
      } catch (err) {
        setSetupStatus('Import failed: ' + err);
      }
    });
    setupEl.appendChild(devButton);
  }

  // --- Settings -------------------------------------------------------------
  //
  // The engine reads its configuration from two ini files in the user root,
  // Game.ini and User.ini, at startup (Core/Src/UnMisc.cpp). The page already
  // owns that root over IDBFS, so the settings panel writes the same keys the
  // native launcher writes (HP2LauncherStore.cpp SaveSettings) and the engine
  // picks them up on the next launch.
  //
  // Only settings a browser can actually honor are offered. Screen mode,
  // resolution, the Vulkan backend and native text are absent because the
  // page has no say over them; every control here changes engine behavior.
  //
  // Every entry also carries the value the launcher applies on a first run,
  // to a key the player has never set -- see loadSettings. The data root is a
  // fallback for the launcher here, not its peer: the prototype's ini files
  // pin several keys to 2002 hardware assumptions, so a first run would
  // otherwise inherit those instead of the best this engine can do. Columns:
  // what the engine declares, what the prototype actually ships, and what the
  // launcher writes.
  //
  // key                     engine declares        prototype ships   launcher
  // FrameRateLimit          none; UnGame.cpp:1955  60.000000         144
  //                         passes it straight on
  // ShowFPS                 0 (UnCamMgr.cpp:84)    absent            False
  // UIScale                 1.0 (SDLClient.cpp:74) absent            1.0
  // Brightness              0.5 (SDLClient.cpp:65) 0.400000 *        0.4
  // ScreenFlashes           1 (SDLClient.cpp:67)   True (Win/XDrv)   True
  // MaintainVerticalFOV     1 (UnCamMgr.cpp:78)    absent            True
  // UseVSync                VS_Adaptive            absent            On
  //                         (XOpenGL.cpp:268)
  // RenderScale             1 (XOpenGL.cpp:216)    absent            1.0
  // TextureDetail           0 = High               absent            High
  //                         (UnCamMgr.cpp:51)
  // UseAA                   1 (XOpenGL.cpp:226)    absent            True
  // UseSound                True (Engine.uc:32)    True              True
  // SoundVolume             0.9 (Default.ini:157)  0.9               0.9
  // MusicVolume             0.5 (Default.ini:156)  0.5               0.53
  // Difficulty              Medium                 DifficultyEasy    Easy
  //                         (PlayerPawn.uc:5184)   (DefUser.ini:235)
  // MouseSensitivity        3.0                    absent            3.0
  //                         (PlayerPawn.uc:5169)
  // bInvertMouse            False (never set)      absent            False
  // bModernThirdPerson-     False (bitfield,       False             False
  //   Controls              UnPawn.cpp:243)        (DefUser.ini:214)
  // ObjectDetail            VeryHigh               absent            VeryHigh
  //                         (PlayerPawn.uc:5185)
  // bAutoCenterCamera       True (Harry.uc:6352)   True              True
  // bMoveWhileCasting       False (never set)      True              True
  //                                               (DefUser.ini:244)
  // bAutoQuaff              True (Harry.uc:6367)   True              True
  //
  // * Brightness is the one key the launcher deliberately does not take to
  //   the maximum. It is a gamma calibration rather than a quality knob: the
  //   0.4 the prototype carries in its Windows and X client sections is the
  //   value this content was balanced against, and pushing it to 1.0 washes
  //   the whole scene out. The other entries are capability or preference
  //   knobs, so they take the best value the engine offers.

  const SETTINGS = [
    // 144 is the highest refresh the launcher offers
    // (FrameRateLimitValues, SDLLaunch/Src/HP2LauncherModel.h:99) and the
    // engine declares no default of its own for it. This deliberately
    // overwrites a value the data root ships: Default.ini pins
    // FrameRateLimit=60.000000, which outranks anything the engine would
    // declare, so writing 144 here is the only thing that makes it the real
    // first-run cap. A value the player picks is never touched.
    { section: 'Engine.GameEngine', file: 'game', key: 'FrameRateLimit',
      label: 'Frame rate limit', default: '144', type: 'select', values: [
        { text: 'Unlimited', value: '0' }, { text: '30', value: '30' },
        { text: '60', value: '60' }, { text: '120', value: '120' },
        { text: '144', value: '144' } ] },
    // An on-screen frame counter is a debug overlay, not a quality setting,
    // and off is what the engine itself declares (UnCamMgr.cpp:84).
    { section: 'SDLDrv.SDLClient', file: 'game', key: 'ShowFPS',
      label: 'Show FPS', default: 'False', type: 'toggle' },
    // 1.0 is the engine's declared default (SDLClient.cpp:74) and 1:1
    // pixels; the steps above it are a comfort preference, not more
    // capability, and PostEditChange (SDLClient.cpp:121) rejects anything
    // non-positive outright.
    { section: 'SDLDrv.SDLClient', file: 'game', key: 'UIScale',
      label: 'Interface scale', default: '1.0', type: 'select', values: [
        { text: '75%', value: '0.75' }, { text: '100%', value: '1.0' },
        { text: '125%', value: '1.25' }, { text: '150%', value: '1.5' },
        { text: '175%', value: '1.75' }, { text: '200%', value: '2.0' } ] },
    // Gamma calibration, not a quality knob: 0.4 is what the prototype
    // balances against (Default.ini:84,112) and what the native launcher
    // ships (HP2LauncherModel.h:127).
    { section: 'SDLDrv.SDLClient', file: 'game', key: 'Brightness',
      label: 'Brightness', default: '0.4', type: 'range',
      min: 0, max: 1, step: 0.05 },
    // Maximum, and already the engine's default (SDLClient.cpp:67).
    { section: 'SDLDrv.SDLClient', file: 'game', key: 'ScreenFlashes',
      label: 'Screen flashes', default: 'True', type: 'toggle' },
    // Maximum, and the engine's default (UnCamMgr.cpp:78). Off would stretch
    // the authored field of view horizontally on any non-4:3 viewport.
    { section: 'SDLDrv.SDLClient', file: 'game', key: 'MaintainVerticalFOV',
      label: 'Maintain vertical field of view', default: 'True',
      type: 'toggle' },

    // Off by default. The engine's own declared value is VS_Adaptive
    // (XOpenGL.cpp:268), and adaptive resolves to swap interval -1
    // (XOpenGL.cpp:1400), which trades input latency for a smoother frame.
    // Neither is the plain off the default wants, and a page already pacing
    // itself through requestAnimationFrame gains little from capping again.
    // Turning it on is one click away.
    { tab: 'Graphics', section: 'XOpenGLDrv.XOpenGLRenderDevice', file: 'game',
      key: 'UseVSync', label: 'Vertical sync', default: 'Off', type: 'select',
      values: [
        { text: 'Off', value: 'Off' }, { text: 'On', value: 'On' } ] },
    // 1.0, the engine's default (XOpenGL.cpp:216). There is no more detail
    // above it, only a larger framebuffer.
    { tab: 'Graphics', section: 'XOpenGLDrv.XOpenGLRenderDevice', file: 'game',
      key: 'RenderScale', label: 'Render scale', default: '1.0', type: 'select',
      values: [
        { text: '50%', value: '0.5' }, { text: '67%', value: '0.67' },
        { text: '75%', value: '0.75' }, { text: '85%', value: '0.85' },
        { text: '100%', value: '1.0' } ] },
    // High: the top of the engine's Details enum, whose index 0 is the finest
    // mip bias (UnCamMgr.cpp:50-54). The values are that enum's own names on
    // purpose -- UByteProperty::ImportText resolves the token against Details
    // by FName (Core/Src/UnProp.cpp:410-419), so "High" parses and
    // "TextureDetailHigh" does not, which is also what the native launcher
    // writes (HP2LauncherStore.cpp Texture()).
    { tab: 'Graphics', section: 'SDLDrv.SDLClient', file: 'game', key: 'TextureDetail',
      label: 'Texture detail', default: 'High', type: 'select', values: [
        { text: 'Low', value: 'Low' },
        { text: 'Medium', value: 'Medium' },
        { text: 'High', value: 'High' } ] },
    // Maximum, and already the engine's default (XOpenGL.cpp:226).
    { tab: 'Graphics', section: 'SDLDrv.SDLClient', file: 'game', key: 'UseAA',
      label: 'Anti-aliasing', default: 'True', type: 'toggle' },

    // Maximum, the engine's default (Engine/Classes/Engine.uc:32) and what
    // [Engine.GameEngine] already ships.
    { tab: 'Audio', section: 'Engine.GameEngine', file: 'game', key: 'UseSound',
      label: 'Sound', default: 'True', type: 'toggle' },
    // 0.9 is the shipped mix (Default.ini:157) and the launcher's own
    // default (HP2LauncherModel.h:131): the level the effects were balanced
    // to, and anything above it clips.
    { tab: 'Audio', section: 'ALAudio.ALAudioSubsystem', file: 'game',
      key: 'SoundVolume', label: 'Effects volume', default: '0.9',
      type: 'range', min: 0, max: 1, step: 0.05 },
    // 0.53, the launcher's own default (HP2LauncherModel.h:132). Music is
    // meant to sit under the effects mix; the prototype's 0.5 is the same
    // choice a shade lower. The step is 0.01 rather than 0.05 because an
    // input[type=range] rounds an off-step default to the nearest step, which
    // would show 0.55 and hand that to the ini on the first edit.
    { tab: 'Audio', section: 'ALAudio.ALAudioSubsystem', file: 'game',
      key: 'MusicVolume', label: 'Music volume', default: '0.53',
      type: 'range', min: 0, max: 1, step: 0.01 },

    // Easy, matching the prototype's own [Engine.PlayerPawn] and the launcher
    // (HP2LauncherModel.h:141). The pawn declares Medium, but a first run
    // nobody has tuned should not open harder than the content asks.
    { tab: 'Gameplay', section: 'Engine.PlayerPawn', file: 'user', key: 'Difficulty',
      label: 'Difficulty', default: 'DifficultyEasy', type: 'select', values: [
        { text: 'Easy', value: 'DifficultyEasy' },
        { text: 'Medium', value: 'DifficultyMedium' },
        { text: 'Hard', value: 'DifficultyHard' } ] },
    // 3.0, the pawn's declared default (PlayerPawn.uc:5169) and the
    // launcher's: a fresh install has no basis for any other figure.
    { tab: 'Gameplay', section: 'Engine.PlayerPawn', file: 'user', key: 'MouseSensitivity',
      label: 'Mouse sensitivity', default: '3.0', type: 'range',
      min: 0.1, max: 10, step: 0.1 },
    // The pawn never sets it and the launcher defaults it off
    // (HP2LauncherModel.h:134).
    { tab: 'Gameplay', section: 'Engine.PlayerPawn', file: 'user', key: 'bInvertMouse',
      label: 'Invert mouse', default: 'False', type: 'toggle' },
    // False, and deliberately so: this swaps in a movement model the shipped
    // content was not built around, and both the prototype
    // (DefUser.ini:214) and the launcher (Classic, HP2LauncherModel.h:135)
    // choose it.
    { tab: 'Gameplay', section: 'Engine.PlayerPawn', file: 'user',
      key: 'bModernThirdPersonControls', label: 'Modern controls',
      default: 'False', type: 'toggle' },
    // ObjectDetailVeryHigh: top of EObjectDetail (PlayerPawn.uc:123-130) and
    // already the pawn's own default (PlayerPawn.uc:5185).
    { tab: 'Gameplay', section: 'Engine.PlayerPawn', file: 'user', key: 'ObjectDetail',
      label: 'Object detail', default: 'ObjectDetailVeryHigh', type: 'select', values: [
        { text: 'Very low', value: 'ObjectDetailVeryLow' },
        { text: 'Low', value: 'ObjectDetailLow' },
        { text: 'Medium', value: 'ObjectDetailMedium' },
        { text: 'High', value: 'ObjectDetailHigh' },
        { text: 'Very high', value: 'ObjectDetailVeryHigh' } ] },
    // Maximum, and the engine's own default (Harry.uc:6352).
    { tab: 'Gameplay', section: 'HGame.Harry', file: 'user', key: 'bAutoCenterCamera',
      label: 'Auto-center camera', default: 'True', type: 'toggle' },
    // True: moving while casting is what the engine supports
    // (Harry.uc:5779), and the prototype ships it on (DefUser.ini:244).
    // Harry.uc never sets it, so without this it would be off on a fresh
    // install even though the data root asks for it.
    { tab: 'Gameplay', section: 'HGame.Harry', file: 'user', key: 'bMoveWhileCasting',
      label: 'Move while casting', default: 'True', type: 'toggle' },
    // Maximum, and the engine's default (Harry.uc:6367).
    { tab: 'Gameplay', section: 'HGame.Harry', file: 'user', key: 'bAutoQuaff',
      label: 'Auto-quaff potions', default: 'True', type: 'toggle' },
  ];

  function iniPath(file) {
    return USER_ROOT + '/' + (file === 'user' ? 'User.ini' : 'Game.ini');
  }

  // Rewrites one key inside one section, leaving every other line exactly as
  // it was. The engine's ini files carry the level and package setup, so a
  // wholesale replacement would destroy the install.
  // The engine's ini files can repeat a section: Default.ini has more than one
  // [SDLDrv.SDLClient], and the last occurrence is the one in effect, so both
  // helpers below work on the last one rather than the first. Reading or
  // editing the first would show a stale value for half the settings.
  function lastSectionSpan(lines, section) {
    const header = '[' + section + ']';
    let start = -1;
    for (let i = 0; i < lines.length; ++i) {
      // trim() already drops the CR, so a CRLF ini still matches. The key
      // comparison below is the part that has to strip it explicitly: the
      // engine writes Default.ini with Windows line endings, and a key read as
      // "FrameRateLimit\r" matches nothing, which silently left the control
      // showing the prototype's value.
      if (lines[i].trim() === header) {
        start = i;
      }
    }
    if (start < 0) {
      return null;
    }
    let end = lines.length;
    for (let i = start + 1; i < lines.length; ++i) {
      if (lines[i].trim().startsWith('[')) {
        end = i;
        break;
      }
    }
    return { start: start, end: end };
  }

  function setIniValue(text, section, key, value) {
    const lines = text.split('\n');
    const span = lastSectionSpan(lines, section);
    if (span === null) {
      // Append the section, keeping one blank line between blocks.
      const suffix = text.length === 0 || text.endsWith('\n') ? '' : '\n';
      return text + suffix + '[' + section + ']\n' + key + '=' + value + '\n';
    }
    for (let i = span.start + 1; i < span.end; ++i) {
      const eq = lines[i].indexOf('=');
      if (eq > 0 && lines[i].slice(0, eq).trim().toLowerCase() === key.toLowerCase()) {
        lines[i] = key + '=' + value;
        return lines.join('\n');
      }
    }
    lines.splice(span.end, 0, key + '=' + value);
    return lines.join('\n');
  }

  function readIniValue(text, section, key) {
    const lines = text.split('\n');
    const span = lastSectionSpan(lines, section);
    if (span === null) {
      return null;
    }
    for (let i = span.start + 1; i < span.end; ++i) {
      const eq = lines[i].indexOf('=');
      if (eq > 0 && lines[i].slice(0, eq).trim().toLowerCase() === key.toLowerCase()) {
        return lines[i].slice(eq + 1).trim();
      }
    }
    return null;
  }

  // Copy the data root's ini prototypes into the user root, which is what the
  // engine does at startup (Core/Src/UnMisc.cpp). Doing it before the settings
  // panel edits them means a first launch has the engine's real defaults in
  // place; without it the panel would write a three-key file that silently
  // drops the package and level setup.
  function dataRootReady() {
    if (!currentModule || !currentModule.FS) {
      return false;
    }
    try {
      currentModule.FS.stat(DATA_ROOT + '/System');
      return true;
    } catch (err) {
      return false;
    }
  }

  function userIniPresent(file) {
    if (!currentModule || !currentModule.FS) {
      return false;
    }
    try {
      currentModule.FS.stat(iniPath(file || 'game'));
      return true;
    } catch (err) {
      return false;
    }
  }

  // Pulls the two ini prototypes straight out of the import. They are tiny and
  // the user root is empty before the first launch, so this lets the panel be
  // used straight after an import without waiting for a 500 MB copy; the
  // launch still seeds from the data root once it has it.
  async function seedUserIniFromImport() {
    if (!currentModule || !currentModule.FS) {
      return;
    }
    try {
      const opfsRoot = await navigator.storage.getDirectory();
      const dataDir = await opfsRoot.getDirectoryHandle(OPFS_DIR);
      const systemDir = await dataDir.getDirectoryHandle('System');
      for (const pair of [['Default.ini', 'game'], ['DefUser.ini', 'user']]) {
        if (userIniPresent(pair[1])) {
          continue;
        }
        const handle = await systemDir.getFileHandle(pair[0]);
        const buffer = await (await handle.getFile()).arrayBuffer();
        currentModule.FS.writeFile(iniPath(pair[1]), new Uint8Array(buffer));
      }
    } catch (err) {
      // No import yet, or it has not finished: the launch will seed instead.
    }
  }

  function seedUserIniFromData() {
    if (!currentModule || !currentModule.FS) {
      return;
    }
    for (const pair of [['Default.ini', 'game'], ['DefUser.ini', 'user']]) {
      const target = iniPath(pair[1]);
      const source = DATA_ROOT + '/System/' + pair[0];
      let prototypeText = '';
      try {
        prototypeText = new TextDecoder('utf-8')
          .decode(currentModule.FS.readFile(source));
      } catch (err) {
        continue;
      }
      let currentText = null;
      try {
        currentText = new TextDecoder('utf-8')
          .decode(currentModule.FS.readFile(target));
      } catch (err) {
        currentText = null;
      }
      // Seed when absent, and also when what is on disk has lost ground
      // against the prototype: every section has to survive, because the
      // engine needs [Core.System] Paths= to find the packages at all
      // (Core/Src/UnUnix.cpp appPlatformInit) and a truncated file is
      // exactly how a game stops starting. Counting sections rather than
      // testing the key directly is what keeps a section the page has
      // legitimately edited from being reverted on every launch.
      const sections = (text) => (text.match(/^\[/gm) || []).length;
      if (currentText === null ||
          sections(currentText) < sections(prototypeText)) {
        currentModule.FS.writeFile(target, prototypeText);
      }
    }
  }

  // The values in front of the engine, keyed by ini key. loadSettings
  // resolves each one -- the player's value when the user ini holds something
  // the data root does not, the launcher's default otherwise -- and
  // saveSettings writes them all back. There is no third state and no
  // "already set" skip, because that is what would silently defeat the whole
  // feature: seedUserIniFromData copies Default.ini into the user root, so a
  // first run looks exactly like a returning one, and anything keyed on
  // "the ini already has this key" leaves FrameRateLimit at the prototype's
  // 60.000000. Writing every resolved value is idempotent, so a value the
  // player set comes back from a reload unchanged.
  const settingsState = {};

  // One reader for both the load and the save path, so a decoding quirk
  // cannot show up in one and not the other. A missing file falls back to the
  // data root's prototype -- which is what the engine itself copies at startup
  // -- so writing into a fresh install edits the real defaults instead of a
  // near-empty file that drops the package and level setup.
  function readIniFile(file) {
    return readIniText(iniPath(file),
      DATA_ROOT + '/System/' + (file === 'user' ? 'DefUser.ini' : 'Default.ini'));
  }

  // Reads one ini, falling back to its prototype. The fallback is what the
  // engine itself does at startup, so a fresh install behaves the same way.
  function readIniText(path, prototype) {
    const decode = function (target) {
      return new TextDecoder('utf-8').decode(currentModule.FS.readFile(target));
    };
    try {
      return decode(path);
    } catch (err) {
      if (!prototype) {
        return '';
      }
      try {
        return decode(prototype);
      } catch (err2) {
        return '';
      }
    }
  }

  // One reader for both the load and the save path, so a decoding quirk
  // cannot show up in one and not the other. A missing file falls back to the
  // data root's prototype -- which is what the engine itself copies at startup
  // -- so writing into a fresh install edits the real defaults instead of a
  // near-empty file that drops the package and level setup.
  function readIniFile(file) {
    return readIniText(iniPath(file),
      DATA_ROOT + '/System/' + (file === 'user' ? 'DefUser.ini' : 'Default.ini'));
  }

  // Reads one ini, falling back to its prototype. The fallback is what the
  // engine itself does at startup, so a fresh install behaves the same way.
  function readIniText(path, prototype) {
    const decode = function (target) {
      return new TextDecoder('utf-8').decode(currentModule.FS.readFile(target));
    };
    try {
      return decode(path);
    } catch (err) {
      if (!prototype) {
        return '';
      }
      try {
        return decode(prototype);
      } catch (err2) {
        return '';
      }
    }
  }

  // The launcher's baseline, read from the data root itself and never from the
  // user ini -- comparing a stored value against a baseline that came from
  // the same file would always match, and the defaults would never be written.
  // No prototype to fall back to, hence no fallback argument.
  // The prototype ini lives in the data root, which only exists in MEMFS after
  // a launch has copied the import in. Before that first launch it is still in
  // OPFS, so read it from there when the module's filesystem does not have it:
  // otherwise a player who imports and then opens the panel sees no defaults,
  // because every key resolves against a missing prototype.
  let prototypeCache = null;

  async function prototypeText(file) {
    const name = file === 'user' ? 'DefUser.ini' : 'Default.ini';
    if (!prototypeCache) {
      prototypeCache = {};
      const fromModule = readIniText(DATA_ROOT + '/System/' + name, null);
      if (fromModule) {
        prototypeCache[file] = fromModule;
      }
    }
    if (prototypeCache[file]) {
      return prototypeCache[file];
    }
    try {
      const opfsRoot = await navigator.storage.getDirectory();
      const dataDir = await opfsRoot.getDirectoryHandle(OPFS_DIR);
      const systemDir = await dataDir.getDirectoryHandle('System');
      const handle = await systemDir.getFileHandle(name);
      const buffer = await (await handle.getFile()).arrayBuffer();
      prototypeCache[file] = new TextDecoder('utf-8').decode(new Uint8Array(buffer));
    } catch (err) {
      prototypeCache[file] = '';
    }
    return prototypeCache[file];
  }

  // A value the player owns always wins. Everything else -- the data root's
  // own number, or no number at all -- resolves to the launcher's default,
  // because the launcher is the settings authority and the engine reads what
  // it writes. Resolving here is what lets saveSettings put the defaults on
  // disk instead of leaving them implicit. A key already in settingsState is
  // this session's -- a panel choice, or a default an earlier load resolved --
  // and keeps its value, so the re-resolve at launch cannot undo a choice
  // made while the panel was up.
  // The engine writes these numbers as floats ("60.000000") while the panel
  // offers "60". Comparing them as strings makes a player who picks 60 and the
  // prototype's own 60.000000 look different, so the stored value never matches
  // a control and the first run's choice looks like it was lost.
  function sameSettingValue(a, b) {
    if (a === null || b === null || a === undefined || b === undefined) {
      return false;
    }
    const left = String(a).trim();
    const right = String(b).trim();
    if (left === '' || right === '') {
      return left === right;
    }
    const leftNumber = Number(left);
    const rightNumber = Number(right);
    if (Number.isFinite(leftNumber) && Number.isFinite(rightNumber)) {
      return leftNumber === rightNumber;
    }
    return left.toLowerCase() === right.toLowerCase();
  }

  function loadSettings() {
    if (!currentModule || !currentModule.FS) {
      return;
    }
    const game = readIniFile('game');
    const user = readIniFile('user');
    for (const setting of SETTINGS) {
      if (Object.prototype.hasOwnProperty.call(settingsState, setting.key)) {
        continue;
      }
      const value = readIniValue(
        setting.file === 'user' ? user : game, setting.section, setting.key);
      // The user ini holding something the data root does not is what makes a
      // value the player's: they set it, or an earlier run of this page did
      // on their behalf. Anything else is the prototype's own number or no
      // number at all, and the launcher's default replaces it -- keeping the
      // prototype's number would write the very value the launcher exists to
      // improve on.
      settingsState[setting.key] =
        value !== null && !sameSettingValue(value, setting.default)
          ? value : setting.default;
    }
  }

  // Writes every resolved value, which is what puts the defaults in front of
  // the engine: an engine that never sees FrameRateLimit falls back to the
  // data root's 60, so merely showing 144 in the panel would be a lie. This
  // is idempotent rather than conditional. Skipping keys the ini already
  // defines looks tidier, but the seed above has just put the prototype's own
  // keys there, so a first run would skip every one of them and the whole
  // change would do nothing. A value the player set is in settingsState
  // already, so it is rewritten with the value it already has.
  async function saveSettings() {
    if (!currentModule || !currentModule.FS) {
      return;
    }
    // The ini prototypes are needed before the first launch can copy the whole
    // data root in. They are two small files and the import already has them in
    // OPFS, so seeding from there keeps a choice made before the first launch
    // from being lost to a reload. The full 500 MB copy is still the launch's
    // job, and the seed only runs when the user root is empty.
    if (!userIniPresent()) {
      seedUserIniFromImport();
    }
    if (!userIniPresent()) {
      return;
    }
    // One base text per file, read once. readIniFile -- not a private
    // reader -- is what makes this safe on a fresh install: it falls back to
    // the data root's prototype, so what the keys below are spliced into is
    // the real ini rather than an empty one.
    const bases = { game: readIniFile('game'), user: readIniFile('user') };
    // A file with no prototype behind it is left untouched. setIniValue
    // appends to whatever base it is handed, so building on an empty one
    // would leave an ini holding nothing but these settings keys: no
    // [Core.System] Paths=, no package setup, and a game that will not
    // start. The engine cannot reach this point without its ini -- the same
    // read is what raises "MisingIni" (Core/Src/UnMisc.cpp:2041) -- so this
    // closes the shape of the bug rather than a path anyone reaches.
    const out = { game: null, user: null };
    const base = function (file) {
      if (bases[file] === '') {
        return '';
      }
      if (out[file] === null) {
        out[file] = bases[file];
      }
      return out[file];
    };
    for (const setting of SETTINGS) {
      const value = settingsState[setting.key];
      if (value === undefined) {
        continue;
      }
      const current = base(setting.file);
      if (current === '') {
        continue;
      }
      out[setting.file] = setIniValue(current, setting.section, setting.key, value);
    }
    for (const file of ['game', 'user']) {
      if (out[file] !== null) {
        currentModule.FS.writeFile(iniPath(file), out[file]);
        // The engine regenerates Game.ini and User.ini when they are older
        // than the data root's Default.ini and DefUser.ini, which would
        // silently discard everything the player just set -- defaults
        // included. Touching them makes them newer, so the prototypes are
        // only a fallback.
        try {
          const now = new Date();
          currentModule.FS.utimes(iniPath(file), now, now);
        } catch (err) {
          // utimes is optional; without it the engine may still refresh the
          // files from their prototypes, which is a safe default.
        }
      }
    }
  }

  const tabsEl = document.getElementById('tabs');
  const tabPanes = {};
  Array.prototype.forEach.call(document.querySelectorAll('.tab'), function (tab) {
    tabPanes[tab.dataset.tab] = document.getElementById('tab-' + tab.dataset.tab);
  });

  // Which pane each setting belongs in: the Play tab has none, the rest are
  // named by the setting's tab property, and display ones default to Display.
  function paneFor(setting) {
    // SETTINGS uses title-case tab names ("Graphics") while the element ids are
    // lower-case; normalise here instead of requiring the two to agree, which
    // silently sent every non-Display row to the Display pane.
    const name = (setting.tab || 'display').toLowerCase();
    return document.getElementById('tab-' + name);
  }

  function buildSettingsPanes() {
    for (const name of Object.keys(tabPanes)) {
      const pane = tabPanes[name];
      if (name === 'play') {
        continue;
      }
      const heading = document.createElement('div');
      heading.className = 'label';
      heading.textContent = name === 'display' ? 'Display' : name;
      pane.appendChild(heading);
    }
    for (const setting of SETTINGS) {
      const pane = paneFor(setting);
      if (!pane) {
        continue;
      }
      pane.appendChild(buildSettingRow(setting));
    }
  }

  function buildSettingRow(setting) {
    const row = document.createElement('div');
    row.className = 'field';
    const label = document.createElement('label');
    label.textContent = setting.label;
    row.appendChild(label);

    // Every control routes through here, so the resolved state is the one and
    // only place a change goes: the value is replaced, then written. Nothing
    // is marked or counted, so a later load cannot mistake a choice for a
    // value the data root happened to ship.
    const choose = function (value) {
      settingsState[setting.key] = value;
      return saveSettings();
    };

    if (setting.type === 'select') {
      const select = document.createElement('select');
      for (const option of setting.values) {
        const element = document.createElement('option');
        element.value = option.value;
        element.textContent = option.text;
        select.appendChild(element);
      }
      select.addEventListener('change', function () {
        choose(select.value);
      });
      row.appendChild(select);
      // Every branch must assign the control: applySettingsToControls skips any
      // row whose control is unset, and without this the selects silently kept
      // their first option instead of the resolved value.
      setting.control = select;
    } else if (setting.type === 'range') {
      const input = document.createElement('input');
      input.type = 'range';
      input.min = setting.min;
      input.max = setting.max;
      input.step = setting.step;
      const readout = document.createElement('span');
      readout.className = 'value';
      input.addEventListener('input', function () {
        readout.textContent = input.value;
      });
      input.addEventListener('change', function () {
        choose(input.value);
      });
      row.appendChild(input);
      row.appendChild(readout);
      // The control must be the input itself, not the row's last child: for a
      // slider that is the readout <span>, and writing the resolved value there
      // left the slider sitting at its own default.
      setting.control = input;
      setting.readout = readout;
    } else {
      const wrap = document.createElement('span');
      wrap.className = 'switch';
      const input = document.createElement('input');
      input.type = 'checkbox';
      const track = document.createElement('span');
      track.className = 'track';
      wrap.appendChild(input);
      wrap.appendChild(track);
      input.addEventListener('change', function () {
        choose(input.checked ? 'True' : 'False');
      });
      row.appendChild(wrap);
      setting.control = input;
    }
    setting.labelEl = label;
    return row;
  }

  // Push resolved values into the controls. loadSettings has already replaced
  // every undefined with the launcher's default, so this only has to handle
  // what is in settingsState -- and skip a value this build does not offer.
  function applySettingsToControls() {
    for (const setting of SETTINGS) {
      const value = settingsState[setting.key];
      if (value === undefined || !setting.control) {
        continue;
      }
      if (setting.type === 'select') {
        const select = setting.control;
        select.value = value;
        if (!select.value) {
          // The engine stores these numbers as floats ("60.000000") and a
          // <select> rejects a value it has no option for, leaving it blank.
          // Find the option with the same number and take its own spelling,
          // which is also what gets written back, so the next load compares
          // like with like.
          for (const option of select.options) {
            if (sameSettingValue(option.value, value)) {
              select.value = option.value;
              break;
            }
          }
        }
        if (select.value) {
          settingsState[setting.key] = select.value;
        }
        // Nothing matched: leave the first option rather than showing a blank.
        if (!select.value) {
          select.selectedIndex = 0;
        }
      } else if (setting.type === 'range') {
        setting.control.value = value;
        if (setting.readout) {
          setting.readout.textContent = value;
        }
      } else {
        setting.control.checked = /^(true|1|on|yes)$/i.test(value);
      }
    }
  }


  Array.prototype.forEach.call(document.querySelectorAll('.tab'), function (tab) {
    tab.addEventListener('click', function () {
      const name = tab.dataset.tab;
      Array.prototype.forEach.call(document.querySelectorAll('.tab'), function (other) {
        other.classList.toggle('active', other === tab);
      });
      for (const key of Object.keys(tabPanes)) {
        tabPanes[key].hidden = key !== name;
      }
    });
  });

  function showSetup() {
    setupEl.hidden = false;
    chooserEl.hidden = true;
    tabsEl.hidden = true;
  }

  function showChooser() {
    setupEl.hidden = true;
    chooserEl.hidden = false;
    tabsEl.hidden = false;
    loadSettings();
    applySettingsToControls();
    refreshSaves();
    updateButtons();
  }

  function setSetupStatus(text) {
    setupStatus.textContent = text;
  }

  logToggle.addEventListener('click', function () {
    logEl.classList.toggle('open');
    logToggle.textContent = logEl.classList.contains('open') ? 'Hide log' : 'Log';
  });

  newGameButton.addEventListener('click', function () {
    launch(newGameArgs());
  });

  continueButton.addEventListener('click', function () {
    const args = continueArgs();
    if (args) {
      launch(args);
    }
  });

  // Fullscreen is the page's to give and take: the browser owns the canvas
  // size, so the shell asks for the element and lets the engine's own
  // SDL_WINDOWEVENT_RESIZED path pick the new resolution up. Requesting the
  // document rather than the canvas fills the screen and hides the toolbar.
  const stageEl = document.getElementById('stage');
  const fullscreenToggle = document.getElementById('fullscreen-toggle');

  // The glyph shows the current state; the tooltip also says how to free the
  // mouse when the engine holds the pointer lock and the button cannot be
  // clicked (see syncPointerLockHint below).
  function syncFullscreenButton() {
    fullscreenToggle.textContent = document.fullscreenElement ? '⤡' : '⤢';
    syncPointerLockHint();
  }

  // The engine grabs the mouse while the game runs, and a locked pointer sends
  // every event to the locked element, so neither the button nor a keypress
  // can reach the page until the lock is dropped. Escape is the player's way
  // out; the same release is what makes these controls usable, and the engine
  // re-grabs the pointer when the player clicks back into the game.
  function releasePointerLock() {
    if (document.pointerLockElement && document.exitPointerLock) {
      document.exitPointerLock();
      return true;
    }
    return false;
  }

  async function toggleFullscreen() {
    if (document.fullscreenElement) {
      try {
        await document.exitFullscreen();
      } catch (err) {
        logToBoth('Leaving full screen was refused by the browser: ' + err);
      }
      return;
    }
    const released = releasePointerLock();
    try {
      // A user gesture is required, and the element must be in the document:
      // the stage is display:none until the game starts.
      await document.documentElement.requestFullscreen();
    } catch (err) {
      // Safari on iOS only fullscreens video, and a denied request is normal
      // when the page is not focused; the button simply stays as it was.
      logToBoth('Full screen was refused by the browser: ' + err);
      if (released) {
        // Put the cursor back so the player is not left without a mouse.
        canvas.focus();
      }
    }
  }

  fullscreenToggle.addEventListener('click', toggleFullscreen);
  document.addEventListener('fullscreenchange', syncFullscreenButton);
  syncFullscreenButton();

  // F toggles fullscreen from the keyboard, which also works while the engine
  // holds the pointer lock, because key events still reach the document.
  document.addEventListener('keydown', function (event) {
    if (!started || event.ctrlKey || event.metaKey || event.altKey) {
      return;
    }
    if (event.key === 'f' || event.key === 'F') {
      event.preventDefault();
      toggleFullscreen();
    }
  });

  // Escape means one thing at a time. The engine binds it to "quit"
  // (System/DefUser.ini:58) and the browser reserves it to leave full screen,
  // so pressing it while full screen must not also reach the game and start a
  // quit prompt. It is swallowed here and the game only ever sees it when the
  // page is not full screen.
  document.addEventListener('keydown', function (event) {
    if (event.key !== 'Escape') {
      return;
    }
    if (document.fullscreenElement) {
      event.preventDefault();
      event.stopPropagation();
    }
  }, true);

  // Browser chrome must never win a click the game is using. The canvas
  // already suppresses the context menu, but text selection, the tab key and
  // middle-click paste all leak out of the canvas otherwise, and on a
  // trackpad a stray gesture lands on the browser's own buttons.
  canvas.addEventListener('contextmenu', function (event) {
    event.preventDefault();
  });
  canvas.addEventListener('dragstart', function (event) {
    event.preventDefault();
  });
  canvas.addEventListener('mousedown', function (event) {
    // Middle click pastes and autoscrolls in most browsers; on a mouse the
    // player is aiming, not pasting.
    if (event.button === 1) {
      event.preventDefault();
    }
  });
  canvas.addEventListener('wheel', function (event) {
    // The game reads the wheel itself; stop the page from zooming behind it.
    event.preventDefault();
  }, { passive: false });
  // Nothing in the game is a form control, so Tab must move focus nowhere
  // rather than off the canvas and into the browser's focus ring.
  document.addEventListener('keydown', function (event) {
    if (event.key === 'Tab') {
      event.preventDefault();
    }
  });

  // While the engine holds the pointer lock the button cannot be clicked --
  // every pointer event goes to the locked canvas -- so say so on the button
  // itself rather than leaving a control that looks broken. Pressing Escape
  // (or F) frees the cursor and the button works again.
  function syncPointerLockHint() {
    fullscreenToggle.title = document.pointerLockElement
      ? (document.fullscreenElement
          ? 'Leave full screen (F)'
          : 'Full screen (F, or press Escape to free the mouse)')
      : (document.fullscreenElement ? 'Leave full screen (F)' : 'Full screen (F)');
  }
  document.addEventListener('pointerlockchange', syncPointerLockHint);
  syncPointerLockHint();

  // The corner buttons fade while playing so they never sit in the picture,
  // and come back on hover, focus, or when the pointer is near the top edge.
  function wakeCorners() {
    stageEl.classList.remove('idle');
  }
  let cornerTimer = null;
  function scheduleIdle() {
    wakeCorners();
    clearTimeout(cornerTimer);
    cornerTimer = setTimeout(function () {
      stageEl.classList.add('idle');
    }, 2500);
  }
  canvas.addEventListener('pointermove', function (event) {
    if (event.clientY < 90) {
      wakeCorners();
    } else {
      scheduleIdle();
    }
  });
  fullscreenToggle.addEventListener('focus', wakeCorners);
  logToggle.addEventListener('focus', wakeCorners);
  scheduleIdle();

  canvas.addEventListener('click', function () {
    canvas.focus();
  });

  // Headless boot smoke: import the served data, launch, and report once the
  // engine's main loop has produced frames. Only reachable via ?smoke=boot.
  async function runBootSmoke() {
    // The smoke is a single run against a throwaway browser profile, so nothing
    // it writes needs to survive. Going through the normal import would push
    // half a gigabyte into OPFS and then read it straight back out again on
    // launch; writing the fetched files into the module's filesystem directly
    // halves the work and skips the second copy entirely. The panel is never
    // shown, so there is nothing for the missing import to break.
    try {
      await copyDevServerDataIntoFs(await entriesFromDevServer());
    } catch (err) {
      setStatus('Import failed: ' + err);
      reportSmoke('failed', 'import failed: ' + err);
      return;
    }
    await launch(newGameArgs());
  }

  // The engine logs "Entering main loop." from its print hook; 300 animation
  // frames after that is a rendered frame rate, not just a reached loop.
  let mainLoopSeen = false;
  let smokeFrames = 0;
  function noteOutput(text) {
    logToBoth(text);
    if (mainLoopSeen || smokeMode !== 'boot') {
      return;
    }
    if (String(text).indexOf('Entering main loop.') < 0) {
      return;
    }
    mainLoopSeen = true;
    const tick = function () {
      ++smokeFrames;
      if (smokeFrames < 300) {
        requestAnimationFrame(tick);
        return;
      }
      reportSmoke('running', 'main loop ran ' + smokeFrames + ' frames',
        { frames: smokeFrames });
    };
    requestAnimationFrame(tick);
  }

  // Instantiate at page init rather than on first click: the launcher's
  // choices (does a save exist?) are only answerable once the IDBFS user
  // root is mounted, and the runtime does not run main on its own
  // (INVOKE_RUN=0), so nothing runs until a launch is requested.
  function createModule() {
    return createHP2Module({
      canvas: canvas,
      // argv[0] becomes GModule, which SDLLaunch uses for the log file name;
      // this keeps it HarryPotter2.log like every other platform.
      thisProgram: 'HarryPotter2',
      // EXIT_RUNTIME=0 keeps the C runtime alive after main returns, but the
      // module also needs the matching JS-side flag or it tears down anyway and
      // the requestAnimationFrame callback never runs.
      noExitRuntime: true,
      print: noteOutput,
      printErr: noteOutput,
      onHP2Exit: function (code) {
        finish('Game exited (code ' + code + ').');
      },
      onExit: function (code) {
        finish('Game exited (code ' + code + ').');
      },
      onAbort: function (what) {
        // Emscripten calls onAbort with an empty reason when the runtime tears
        // down after a clean exit. Reporting that as a crash tells the player
        // their game broke when it did not, and the real exit has already
        // been reported through onHP2Exit.
        if (!what) {
          finish('Game exited.');
          return;
        }
        finish('Game crashed: ' + what + '.');
      },
      preRun: [
        function (m) {
          m.ENV.XDG_DATA_HOME = HOME_PREFIX;
          mkdirTree(m.FS, USER_ROOT);
          m.FS.mount(m.FS.filesystems.IDBFS, {}, USER_ROOT);
          m.addRunDependency('hp2-idbfs');
          m.FS.syncfs(true, function (err) {
            if (err) {
              logToBoth('IDBFS load failed: ' + err);
            }
            m.removeRunDependency('hp2-idbfs');
          });
        },
      ],
    });
  }

  (async function init() {
    // The chooser is the product's face, so it is what the page starts on.
    // The setup step replaces it only when there is nothing to play yet.
    chooserEl.hidden = true;
    setupEl.hidden = true;
    if (!self.crossOriginIsolated) {
      setSetupStatus(COOP_STATUS);
      setupEl.hidden = false;
      chooseFolderButton.disabled = true;
      reportSmoke('failed', COOP_STATUS);
      return;
    }
    if (!('getDirectory' in navigator.storage)) {
      setSetupStatus('This browser has no private file storage, so the game cannot be installed here.');
      setupEl.hidden = false;
      chooseFolderButton.disabled = true;
      reportSmoke('failed', 'no origin-private file system (OPFS)');
      return;
    }
    setSetupStatus('Loading the game…');
    let instance;
    try {
      instance = await createModule();
    } catch (err) {
      setSetupStatus('Loading the game failed: ' + err);
      setupEl.hidden = false;
      reportSmoke('failed', 'module instantiation failed: ' + err);
      return;
    }
    // createHP2Module returns a Promise; the resolved instance is what
    // carries FS, so persistence must be wired to that, not to the Promise.
    // It is also published on the page so the audio context state can be
    // read and a suspended context resumed from a real user gesture.
    window.hp2Module = instance;
    moduleReady = true;
    startPersistence(instance);
    // The controls are built once the module is up, because applying stored
    // values needs the ini files, which live inside its filesystem.
    buildSettingsPanes();
    dataImported = await hasMarker();
    if (dataImported) {
      showChooser();
    } else {
      setSetupStatus('');
      showSetup();
    }
    if (smokeMode === 'boot') {
      // Headless Chrome never clicks: boot mode drives the launch itself.
      await runBootSmoke();
      return;
    }
    reportSmoke('ready', 'module instantiated');
  })();
})();
