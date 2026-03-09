import { useEffect, useState, useCallback } from "react";
import type { Settings as SettingsType } from "../types/models";
import { getSettings, updateSettings } from "../lib/commands";
import LocationSettings from "../components/LocationSettings";
import TaxaPicker from "../components/TaxaPicker";
import DisplaySettings from "../components/DisplaySettings";
import ContentFilters from "../components/ContentFilters";
import CacheStatusPanel from "../components/CacheStatus";

export default function Settings() {
  const [settings, setSettings] = useState<SettingsType | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    getSettings()
      .then(setSettings)
      .catch((e) => setError(String(e)));
  }, []);

  const save = useCallback(
    async (updated: SettingsType) => {
      setSettings(updated);
      setSaving(true);
      try {
        await updateSettings(updated);
      } catch (e) {
        setError(String(e));
      } finally {
        setSaving(false);
      }
    },
    [],
  );

  if (error) {
    return (
      <div className="rounded-lg border border-red-800 bg-red-900/30 p-4 text-red-300">
        <p className="font-medium">Error loading settings</p>
        <p className="mt-1 text-sm">{error}</p>
      </div>
    );
  }

  if (!settings) {
    return (
      <div className="py-12 text-center text-gray-400">
        Loading settings...
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {saving && (
        <div className="text-xs text-gray-500">Saving...</div>
      )}

      <LocationSettings settings={settings} onUpdate={save} />
      <TaxaPicker settings={settings} onUpdate={save} />
      <DisplaySettings settings={settings} onUpdate={save} />
      <ContentFilters settings={settings} onUpdate={save} />
      <CacheStatusPanel settings={settings} onUpdate={save} />
    </div>
  );
}
