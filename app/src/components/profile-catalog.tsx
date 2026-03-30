import { useState, useEffect, useCallback } from "react";
import type { Profile } from "../types";
import { ProfileCard } from "./profile-card";
import { ProfileEditor } from "./profile-editor";

const API_BASE = "http://localhost:8008";

interface ProfileCatalogProps {
  isOpen: boolean;
  onClose: () => void;
  onSelect: (profileId: string) => void;
  onCreateNew: () => void;
}

type View = "list" | "editor";

export function ProfileCatalog({
  isOpen,
  onClose,
  onSelect,
  onCreateNew: _onCreateNew,
}: ProfileCatalogProps) {
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [loading, setLoading] = useState(false);
  const [view, setView] = useState<View>("list");
  const [editingId, setEditingId] = useState<string | null>(null);

  const loadProfiles = useCallback(async () => {
    setLoading(true);
    try {
      const res = await fetch(`${API_BASE}/profiles`);
      if (res.ok) setProfiles(await res.json());
    } catch { /* ignore */ }
    setLoading(false);
  }, []);

  useEffect(() => {
    if (isOpen) {
      setView("list");
      setEditingId(null);
      void loadProfiles();
    }
  }, [isOpen, loadProfiles]);

  useEffect(() => {
    if (!isOpen) return;
    const handleKey = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  const handleCreate = () => {
    setEditingId(null); // null = create new
    setView("editor");
  };

  const handleEdit = (profileId: string) => {
    setEditingId(profileId);
    setView("editor");
  };

  const handleDelete = async (profileId: string) => {
    if (!confirm("Delete this profile?")) return;
    try {
      await fetch(`${API_BASE}/profiles/${profileId}`, { method: "DELETE" });
      await loadProfiles();
    } catch { /* ignore */ }
  };

  const handleEditorSave = () => {
    setView("list");
    setEditingId(null);
    void loadProfiles();
  };

  const handleSelect = (profileId: string) => {
    onSelect(profileId);
    onClose();
  };

  return (
    <div className="catalog-overlay" onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <div className="catalog-modal" role="dialog" aria-modal="true">
        {view === "list" ? (
          <>
            <div className="catalog-header">
              <h2>Face Profiles</h2>
              <button className="catalog-close" onClick={onClose}>x</button>
            </div>

            <div className="catalog-toolbar">
              <button className="btn primary catalog-create-btn" onClick={handleCreate}>
                + New Profile
              </button>
            </div>

            <div className="catalog-body">
              {loading && <div className="catalog-status">Loading...</div>}

              {!loading && profiles.length === 0 && (
                <div className="catalog-status">
                  No profiles yet. Click "+ New Profile" to create one.
                </div>
              )}

              {!loading && profiles.length > 0 && (
                <div className="catalog-grid">
                  {profiles.map((p) => (
                    <ProfileCard
                      key={p.id}
                      profile={p}
                      onSelect={handleSelect}
                      onEdit={handleEdit}
                      onDelete={handleDelete}
                    />
                  ))}
                </div>
              )}
            </div>
          </>
        ) : (
          <ProfileEditor
            profileId={editingId}
            onSave={handleEditorSave}
            onCancel={() => { setView("list"); setEditingId(null); }}
          />
        )}
      </div>
    </div>
  );
}
