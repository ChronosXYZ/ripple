import "./global.css";
import { SidebarInset, SidebarProvider, SidebarTrigger } from "./components/ui/sidebar";
import { AppSidebar } from "./components/AppSidebar";
import { Route, Routes } from "react-router";
import { IdentitiesPage } from "./pages/Identities";
import { MessagesPage } from "./pages/Messages";
import { NetworkStatusPage } from "./pages/NetworkStatus";
import { SettingsPage } from "./pages/Settings";
import { AboutPage } from "./pages/About";



function App() {
  // const [greetMsg, setGreetMsg] = useState("");
  // const [name, setName] = useState("");

  // async function greet() {
  //   // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
  //   setGreetMsg(await invoke("greet", { name }));
  // }

  return (
    <main>
      <SidebarProvider>
        <AppSidebar />
        <SidebarInset>
          <SidebarTrigger />
          <div className="flex flex-1 flex-col gap-4 p-4 h-full">
            <Routes>
              <Route path="/" element={<div>Welcome to Bitmessage</div>} />
              <Route path="/identities" element={<IdentitiesPage />} />
              <Route path="/messages" element={<MessagesPage />} />
              <Route path="/network-status" element={<NetworkStatusPage />} />
              <Route path="settings" element={<SettingsPage />} />
              <Route path="/about" element={<AboutPage />} />
            </Routes>
          </div>
        </SidebarInset>
      </SidebarProvider>
    </main>
  );
}

export default App;
