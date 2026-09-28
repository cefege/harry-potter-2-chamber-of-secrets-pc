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
      const dataDir = await opfsRoot.getDirectoryHandle(OPFS_DIR);
      const copied = await copyOpfsDataIntoFs(window.hp2Module, dataDir);
      logToBoth('Loaded ' + copied + ' data files into ' + DATA_ROOT);
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

  function showSetup() {
    setupEl.hidden = false;
    chooserEl.hidden = true;
  }

  function showChooser() {
    setupEl.hidden = true;
    chooserEl.hidden = false;
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

  canvas.addEventListener('click', function () {
    canvas.focus();
  });

  // Headless boot smoke: import the served data, launch, and report once the
  // engine's main loop has produced frames. Only reachable via ?smoke=boot.
  async function runBootSmoke() {
    try {
      await importEntries(await entriesFromDevServer());
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
