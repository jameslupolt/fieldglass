import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import Settings from "./pages/Settings";
import Preview from "./pages/Preview";
import About from "./pages/About";

type Tab = "settings" | "preview" | "about";

const tabs: { id: Tab; label: string }[] = [
  { id: "settings", label: "Settings" },
  { id: "preview", label: "Preview" },
  { id: "about", label: "About" },
];

export default function App() {
  const [activeTab, setActiveTab] = useState<Tab>("settings");

  useEffect(() => {
    const unlisten = listen<string>("navigate", (event) => {
      const tab = event.payload;
      if (tab === "settings" || tab === "preview" || tab === "about") {
        setActiveTab(tab);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return (
    <div className="min-h-screen bg-gray-900 text-gray-100">
      <nav className="flex border-b border-gray-800">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={`px-6 py-3 text-sm font-medium transition-colors ${
              activeTab === tab.id
                ? "border-b-2 border-green-500 text-green-400"
                : "text-gray-400 hover:text-gray-200"
            }`}
          >
            {tab.label}
          </button>
        ))}
      </nav>

      <main className="p-6">
        {activeTab === "settings" && <Settings />}
        {activeTab === "preview" && <Preview />}
        {activeTab === "about" && <About />}
      </main>
    </div>
  );
}
