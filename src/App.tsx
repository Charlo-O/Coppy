import { useState, useEffect, useRef } from 'react';
import './App.css';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

interface ClipboardItem {
  id: string;
  type: 'text' | 'image';
  content: string; // Text content or Image Data URL
  timestamp: number;
  pinned: boolean;
  folderId?: string;
}

interface ClipboardUpdate {
  type: 'text' | 'image';
  content: string;
}

interface FavoriteFolder {
  id: string;
  name: string;
}

interface FavoriteItem {
  id: string;
  type: 'text' | 'image';
  content: string;
  timestamp: number;
  folder_id?: string | null;
}

interface FavoritesState {
  folders: FavoriteFolder[];
  items: FavoriteItem[];
}

// Dummy data for testing
const INITIAL_ITEMS: ClipboardItem[] = [
  { id: '1', type: 'text', content: 'Hello World', timestamp: Date.now(), pinned: false },
  { id: '2', type: 'text', content: 'git commit -m "update"', timestamp: Date.now() - 1000, pinned: false },
  { id: '3', type: 'text', content: 'https://tauri.app', timestamp: Date.now() - 2000, pinned: false },
];

function App() {
  const [query, setQuery] = useState('');
  const [items, setItems] = useState<ClipboardItem[]>(INITIAL_ITEMS);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [folders, setFolders] = useState<FavoriteFolder[]>([]);
  const [activeFolderId, setActiveFolderId] = useState<string>('all');
  const [favoritesLoaded, setFavoritesLoaded] = useState(false);
  const [modal, setModal] = useState<null | { type: 'create-folder' | 'pin-folder' | 'settings'; itemId?: string }>(null);
  const [modalFolderName, setModalFolderName] = useState('');
  const [modalSelectedFolderId, setModalSelectedFolderId] = useState<string>('none');
  const [pendingPinItemId, setPendingPinItemId] = useState<string | null>(null);
  const [autostartEnabled, setAutostartEnabled] = useState(false);
  const [autostartLoading, setAutostartLoading] = useState(true);
  const searchInputRef = useRef<HTMLInputElement>(null);

  // Filter items
  const filteredItems = items.filter(item => {
    if (activeFolderId !== 'all' && item.folderId !== activeFolderId) return false;
    if (query.trim() === '') return true;
    if (item.type !== 'text') return false;
    return item.content.toLowerCase().includes(query.toLowerCase());
  }).sort((a, b) => {
    // Pinned first, then recent
    if (a.pinned && !b.pinned) return -1;
    if (!a.pinned && b.pinned) return 1;
    return b.timestamp - a.timestamp;
  });

  const handleKeyDown = async (e: KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setSelectedIndex(prev => Math.min(prev + 1, filteredItems.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setSelectedIndex(prev => Math.max(prev - 1, 0));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      selectItem(filteredItems[selectedIndex]);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      await getCurrentWebviewWindow().hide();
    } else if (e.key >= '1' && e.key <= '9') {
      e.preventDefault();
      const index = parseInt(e.key) - 1;
      if (index < filteredItems.length) {
        selectItem(filteredItems[index]);
      }
    }
  };

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [filteredItems, selectedIndex]);

  // Load from localStorage on mount
  useEffect(() => {
    const saved = localStorage.getItem('coppy_items');
    if (saved) {
      try {
        const parsed = JSON.parse(saved);
        setItems(parsed);
      } catch (e) { console.error('Failed to load items', e); }
    }
  }, []);

  // Save to localStorage when items change
  useEffect(() => {
    // For now save everything (up to 50 items)
    const toSave = items.filter(i => i.type === 'text').slice(0, 50);
    localStorage.setItem('coppy_items', JSON.stringify(toSave));
  }, [items]);

  useEffect(() => {
    const load = async () => {
      try {
        const state = await invoke<FavoritesState>('load_favorites');
        setFolders(state.folders);
        setItems(prev => {
          const favoriteItems: ClipboardItem[] = state.items.map(i => ({
            id: i.id,
            type: i.type,
            content: i.content,
            timestamp: i.timestamp,
            pinned: true,
            folderId: i.folder_id ?? undefined,
          }));

          const merged = [...favoriteItems];
          for (const it of prev) {
            if (!merged.find(m => m.id === it.id) && !merged.find(m => m.content === it.content)) {
              merged.push(it);
            }
          }
          return merged;
        });
        setFavoritesLoaded(true);
      } catch (e) {
        console.error('Failed to load favorites', e);
        setFavoritesLoaded(true);
      }
    };
    load();
  }, []);

  useEffect(() => {
    const load = async () => {
      try {
        setAutostartLoading(true);
        const enabled = await invoke<boolean>('autostart_is_enabled');
        setAutostartEnabled(enabled);
      } catch (e) {
        console.error('Failed to read autostart state', e);
      } finally {
        setAutostartLoading(false);
      }
    };
    load();
  }, []);

  useEffect(() => {
    if (!favoritesLoaded) return;

    const state: FavoritesState = {
      folders,
      items: items
        .filter(i => i.pinned)
        .map(i => ({
          id: i.id,
          type: i.type,
          content: i.content,
          timestamp: i.timestamp,
          folder_id: i.folderId ?? null,
        })),
    };

    invoke('save_favorites', { state }).catch((e) => {
      console.error('Failed to save favorites', e);
    });
  }, [items, folders, favoritesLoaded]);

  useEffect(() => {
    // Focus search on mount/show
    searchInputRef.current?.focus();

    // Listen to clipboard updates
    const unlistenPromise = listen<ClipboardUpdate>('clipboard-update', (event) => {
      const payload = event.payload;
      setItems(prev => {
        // Avoid duplicates if recent? Or just move to top?
        // If generic "Copy" manager, usually moves to top.
        const existing = prev.find(i => i.content === payload.content);
        if (existing) {
          // Move to top (update timestamp)
          return [
            { ...existing, timestamp: Date.now() },
            ...prev.filter(i => i.id !== existing.id)
          ];
        }
        return [{
          id: Date.now().toString(),
          type: payload.type,
          content: payload.content,
          timestamp: Date.now(),
          pinned: false
        }, ...prev];
      });
    });

    return () => {
      unlistenPromise.then(unlisten => unlisten());
    };
  }, []);

  const selectItem = async (item: ClipboardItem) => {
    try {
      if (item.type === 'text') {
        await invoke('paste_text', { text: item.content });
      } else {
        await invoke('paste_image', { dataUrl: item.content });
      }
    } catch (err) {
      console.error('Failed to copy', err);
    }
  };

  const openCreateFolderModal = (opts?: { pinItemId?: string }) => {
    setModalFolderName('');
    setPendingPinItemId(opts?.pinItemId ?? null);
    setModal({ type: 'create-folder' });
  };

  const openPinFolderModal = (e: React.MouseEvent, item: ClipboardItem) => {
    e.stopPropagation();
    if (item.pinned) {
      setItems(prev => prev.map(it => it.id === item.id ? { ...it, pinned: false, folderId: undefined } : it));
      searchInputRef.current?.focus();
      return;
    }

    setModalSelectedFolderId(item.folderId ?? (activeFolderId !== 'all' ? activeFolderId : 'none'));
    setPendingPinItemId(null);
    setModal({ type: 'pin-folder', itemId: item.id });
  };

  const openSettingsModal = () => {
    setModal({ type: 'settings' });
  };

  const closeModal = () => {
    setModal(null);
    setPendingPinItemId(null);
    searchInputRef.current?.focus();
  };

  const toggleAutostart = async () => {
    try {
      setAutostartLoading(true);
      if (autostartEnabled) {
        await invoke('autostart_disable');
        setAutostartEnabled(false);
      } else {
        await invoke('autostart_enable');
        setAutostartEnabled(true);
      }
    } catch (e) {
      console.error('Failed to toggle autostart', e);
    } finally {
      setAutostartLoading(false);
    }
  };

  const confirmCreateFolder = () => {
    const name = modalFolderName.trim();
    if (!name) return;
    const folder: FavoriteFolder = { id: Date.now().toString(), name };
    setFolders(prev => [folder, ...prev]);
    setActiveFolderId(folder.id);
    if (pendingPinItemId) {
      setItems(prev => prev.map(it => {
        if (it.id !== pendingPinItemId) return it;
        return { ...it, pinned: true, folderId: folder.id };
      }));
    }
    closeModal();
  };

  const confirmPinFolder = () => {
    if (!modal?.itemId) return;
    setItems(prev => prev.map(it => {
      if (it.id !== modal.itemId) return it;
      return {
        ...it,
        pinned: true,
        folderId: modalSelectedFolderId === 'none' ? undefined : modalSelectedFolderId,
      };
    }));
    closeModal();
  };



  return (
    <div className="container">
      <div
        className="title-bar"
        onMouseDown={() => getCurrentWebviewWindow().startDragging()}
      >
        <span className="title-bar-text">Coppy</span>
        <div className="title-bar-actions">
          <button className="title-bar-action" onClick={openSettingsModal}>⚙</button>
          <button
            className="title-bar-close"
            onClick={() => getCurrentWebviewWindow().hide()}
          >×</button>
        </div>
      </div>
      <div className="search-bar">
        <input
          ref={searchInputRef}
          type="text"
          className="search-input"
          placeholder="Search clipboard..."
          value={query}
          onChange={(e) => { setQuery(e.target.value); setSelectedIndex(0); }}
          autoFocus
        />
      </div>
      <div className="folder-bar">
        <div className="folder-tabs">
          <button
            className={`folder-tab ${activeFolderId === 'all' ? 'active' : ''}`}
            onClick={() => { setActiveFolderId('all'); setSelectedIndex(0); }}
          >
            All
          </button>
          {folders.map(f => (
            <button
              key={f.id}
              className={`folder-tab ${activeFolderId === f.id ? 'active' : ''}`}
              onClick={() => { setActiveFolderId(f.id); setSelectedIndex(0); }}
            >
              {f.name}
            </button>
          ))}
        </div>
        <button className="folder-create" onClick={() => openCreateFolderModal()}>＋</button>
      </div>
      <div className="item-list">
        {filteredItems.map((item, index) => (
          <div
            key={item.id}
            className={`item ${index === selectedIndex ? 'selected' : ''} ${item.pinned ? 'pinned' : ''}`}
            onClick={() => selectItem(item)}
            onMouseEnter={() => setSelectedIndex(index)}
          >
            {index < 9 && <div className="item-shortcut">{index + 1}</div>}
            {item.type === 'text' ? (
              <div className="item-content">{item.content}</div>
            ) : (
              <div className="item-content">
                <img className="item-image" src={item.content} />
              </div>
            )}
            <div className="item-pin" onClick={(e) => openPinFolderModal(e, item)}>
              {item.pinned ? '★' : '☆'}
            </div>
          </div>
        ))}
        {filteredItems.length === 0 && (
          <div style={{ padding: '20px', textAlign: 'center', opacity: 0.5 }}>No items found</div>
        )}
      </div>

      {modal && (
        <div className="modal-overlay" onMouseDown={closeModal}>
          <div className="modal" onMouseDown={(e) => e.stopPropagation()}>
            {modal.type === 'create-folder' && (
              <>
                <div className="modal-title">New folder</div>
                <input
                  className="modal-input"
                  value={modalFolderName}
                  onChange={(e) => setModalFolderName(e.target.value)}
                  placeholder="Folder name"
                  autoFocus
                />
                <div className="modal-actions">
                  <button className="modal-btn" onClick={confirmCreateFolder}>OK</button>
                  <button className="modal-btn secondary" onClick={closeModal}>Cancel</button>
                </div>
              </>
            )}

            {modal.type === 'pin-folder' && (
              <>
                <div className="modal-title">Add to folder</div>
                <div className="modal-folder-list">
                  <button
                    className={`modal-folder-item ${modalSelectedFolderId === 'none' ? 'active' : ''}`}
                    onClick={() => setModalSelectedFolderId('none')}
                  >
                    No folder
                  </button>
                  {folders.map(f => (
                    <button
                      key={f.id}
                      className={`modal-folder-item ${modalSelectedFolderId === f.id ? 'active' : ''}`}
                      onClick={() => setModalSelectedFolderId(f.id)}
                    >
                      {f.name}
                    </button>
                  ))}
                </div>
                <div className="modal-actions">
                  <button className="modal-btn" onClick={confirmPinFolder}>OK</button>
                  <button className="modal-btn secondary" onClick={closeModal}>Cancel</button>
                  <button className="modal-btn secondary" onClick={() => openCreateFolderModal({ pinItemId: modal.itemId })}>New folder</button>
                </div>
              </>
            )}

            {modal.type === 'settings' && (
              <>
                <div className="modal-title">Settings</div>
                <div className="settings-row">
                  <div className="settings-label">Launch at startup</div>
                  <button
                    className={`toggle ${autostartEnabled ? 'on' : ''}`}
                    onClick={toggleAutostart}
                    disabled={autostartLoading}
                  >
                    <span className="toggle-thumb" />
                  </button>
                </div>
                <div className="modal-actions">
                  <button className="modal-btn secondary" onClick={closeModal}>Close</button>
                </div>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
