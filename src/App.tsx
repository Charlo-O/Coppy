import { useState, useEffect, useRef } from 'react';

interface ToastState {
  message: string;
  kind: 'error' | 'info';
}
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
  type ModalState =
    | null
    | { type: 'create-folder' }
    | { type: 'pin-folder'; itemId: string }
    | { type: 'settings' }
    | { type: 'edit-text'; itemId: string };

  const [modal, setModal] = useState<ModalState>(null);
  const [modalFolderName, setModalFolderName] = useState('');
  const [modalSelectedFolderId, setModalSelectedFolderId] = useState<string>('none');
  const [pendingPinItemId, setPendingPinItemId] = useState<string | null>(null);
  const [autostartEnabled, setAutostartEnabled] = useState(false);
  const [autostartLoading, setAutostartLoading] = useState(true);
  const [editText, setEditText] = useState('');
  const [toast, setToast] = useState<ToastState | null>(null);
  const toastTimerRef = useRef<number | null>(null);
  const pendingClipboardSetRef = useRef<null | { itemId: string; text: string; expiresAt: number }>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);

  const showToast = (message: string, kind: ToastState['kind'] = 'info') => {
    setToast({ message, kind });
    if (toastTimerRef.current !== null) {
      window.clearTimeout(toastTimerRef.current);
    }
    toastTimerRef.current = window.setTimeout(() => {
      setToast(null);
      toastTimerRef.current = null;
    }, 2600);
  };

  const hideCurrentWindow = async () => {
    try {
      await getCurrentWebviewWindow().hide();
    } catch (err) {
      console.error('Failed to hide window', err);
      showToast('关闭失败', 'error');
    }
  };

  const openEditTextForItem = (item: ClipboardItem) => {
    if (item.type !== 'text') return;
    setEditText(item.content);
    setModal({ type: 'edit-text', itemId: item.id });
  };

  const confirmEditText = async () => {
    if (!modal || modal.type !== 'edit-text') return;

    const itemId = modal.itemId;
    const newText = editText;

    if (newText.trim().length === 0) {
      showToast('内容不能为空', 'error');
      return;
    }

    setItems(prev => prev.map(it => {
      if (it.id !== itemId) return it;
      return { ...it, type: 'text', content: newText, timestamp: Date.now() };
    }));

    const expiresAt = Date.now() + 1500;
    pendingClipboardSetRef.current = { itemId, text: newText, expiresAt };
    window.setTimeout(() => {
      const pending = pendingClipboardSetRef.current;
      if (pending && pending.itemId === itemId && pending.text === newText && Date.now() >= pending.expiresAt) {
        pendingClipboardSetRef.current = null;
      }
    }, 1600);

    closeModal();

    try {
      await invoke('set_clipboard_text', { text: newText });
    } catch (e) {
      console.error('Failed to set clipboard text', e);
      showToast('写入系统剪贴板失败（列表已更新）', 'error');
    }
  };


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
    if (modal) {
      if (e.key === 'Escape') {
        e.preventDefault();
        closeModal();
      }
      return;
    }

    const active = document.activeElement;
    const activeTag = (active && active instanceof HTMLElement) ? active.tagName.toLowerCase() : '';
    const isTyping = activeTag === 'input' || activeTag === 'textarea';

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
      await hideCurrentWindow();
    } else if (e.key >= '1' && e.key <= '9') {
      e.preventDefault();
      const index = parseInt(e.key) - 1;
      if (index < filteredItems.length) {
        selectItem(filteredItems[index]);
      }
    } else if (!isTyping && (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'v') {
      const it = filteredItems[selectedIndex];
      if (!it) return;

      if (it.type === 'image') {
        e.preventDefault();
        try {
          const savedPath = await invoke<string>('save_image_data_url', { dataUrl: it.content });
          showToast(`已保存到 ${savedPath}`);
        } catch (err) {
          console.error('Failed to save image', err);
          showToast('保存图片失败', 'error');
        }
      }
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'e') {
      e.preventDefault();
      const it = filteredItems[selectedIndex];
      if (it && it.type === 'text') openEditTextForItem(it);
    } else if (!isTyping && e.key.toLowerCase() === 'e') {
      e.preventDefault();
      const it = filteredItems[selectedIndex];
      if (it && it.type === 'text') openEditTextForItem(it);
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
        const now = Date.now();

        const pending = pendingClipboardSetRef.current;
        const shouldUsePending =
          pending &&
          pending.expiresAt >= now &&
          payload.type === 'text' &&
          payload.content === pending.text;

        if (shouldUsePending) {
          const pendingItemId = pending.itemId;
          pendingClipboardSetRef.current = null;

          const existingById = prev.find(i => i.id === pendingItemId);
          if (existingById) {
            return [
              { ...existingById, timestamp: now },
              ...prev.filter(i => i.id !== pendingItemId)
            ];
          }
        }

        const existing = prev.find(i => i.content === payload.content);
        if (existing) {
          return [
            { ...existing, timestamp: now },
            ...prev.filter(i => i.id !== existing.id)
          ];
        }
        return [{
          id: now.toString(),
          type: payload.type,
          content: payload.content,
          timestamp: now,
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
        // For images, just copy to clipboard (don't auto-paste)
        // This allows the user to right-click paste wherever they want
        await invoke('set_clipboard_image', { dataUrl: item.content });
        showToast('已复制到剪贴板');
        await hideCurrentWindow();
      }
    } catch (err) {
      console.error('Failed to copy', err);
      showToast('复制失败', 'error');
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

  const onEditTextareaKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
      e.preventDefault();
      void confirmEditText();
    }
  };

  const closeModal = () => {
    setModal(null);
    setPendingPinItemId(null);
    setEditText('');
    searchInputRef.current?.focus();
  };

  const onTitleBarMouseDown = (e: React.MouseEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement;
    if (target.closest('.title-bar-actions')) return;
    void getCurrentWebviewWindow().startDragging();
  };

  const hideWindow = async (e: React.MouseEvent) => {
    e.stopPropagation();
    await hideCurrentWindow();
  };

  const toggleAutostart = async () => {
    try {
      setAutostartLoading(true);
      if (autostartEnabled) {
        await invoke('autostart_disable');
      } else {
        await invoke('autostart_enable');
      }

      const enabled = await invoke<boolean>('autostart_is_enabled');
      setAutostartEnabled(enabled);
      showToast(enabled ? '已开启开机自启' : '已关闭开机自启');
    } catch (e) {
      console.error('Failed to toggle autostart', e);
      const message = typeof e === 'string' ? e : '设置开机自启失败';
      showToast(message, 'error');
      try {
        const enabled = await invoke<boolean>('autostart_is_enabled');
        setAutostartEnabled(enabled);
      } catch {
        // ignore
      }
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
    if (!modal || modal.type !== 'pin-folder') return;
    const itemId = modal.itemId;

    setItems(prev => prev.map(it => {
      if (it.id !== itemId) return it;
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
        onMouseDown={onTitleBarMouseDown}
      >
        <span className="title-bar-text">Coppy</span>
        <div className="title-bar-actions">
          <button className="title-bar-action" onMouseDown={(e) => e.stopPropagation()} onClick={openSettingsModal}>⚙</button>
          <button
            className="title-bar-close"
            onMouseDown={(e) => e.stopPropagation()}
            onClick={hideWindow}
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
            <div className="item-actions">
              {item.type === 'text' && (
                <button
                  className="item-action"
                  onClick={(e) => { e.stopPropagation(); openEditTextForItem(item); }}
                  title="Edit (E)"
                >
                  ✎
                </button>
              )}
              <button
                className="item-action item-pin"
                onClick={(e) => openPinFolderModal(e, item)}
                title={item.pinned ? 'Unpin' : 'Pin'}
              >
                {item.pinned ? '★' : '☆'}
              </button>
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
                  <button
                    className="modal-btn secondary"
                    onClick={() => openCreateFolderModal({ pinItemId: modal.itemId })}
                  >New folder</button>
                </div>
              </>
            )}

            {modal.type === 'edit-text' && (
              <>
                <div className="modal-title">Edit text</div>
                <textarea
                  className="modal-textarea"
                  value={editText}
                  onChange={(e) => setEditText(e.target.value)}
                  onKeyDown={onEditTextareaKeyDown}
                  placeholder="Enter text"
                  autoFocus
                />
                <div className="modal-actions">
                  <button className="modal-btn" onClick={() => void confirmEditText()}>Save</button>
                  <button className="modal-btn secondary" onClick={closeModal}>Cancel</button>
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

      {toast && (
        <div className={`toast ${toast.kind === 'error' ? 'error' : ''}`}>
          {toast.message}
        </div>
      )}
    </div>
  );
}

export default App;
