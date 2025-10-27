import {
    Sidebar,
    SidebarContent,
    SidebarGroup,
    SidebarGroupContent,
    SidebarHeader,
    SidebarMenu,
    SidebarMenuButton,
    SidebarMenuItem,
} from "@/components/ui/sidebar"
import { Fingerprint, Info, Mails, MessageSquareLock, Network, Settings } from "lucide-react"
import { Link } from "react-router"

const items = [
    {
        title: "Identities",
        url: "/identities",
        icon: Fingerprint,
    },
    {
        title: "Messages",
        url: "/messages",
        icon: Mails,
    },
    {
        title: "Network Status",
        url: "/network-status",
        icon: Network,
    },
    {
        title: "Settings",
        url: "/settings",
        icon: Settings,
    },
    {
        title: "About",
        url: "/about",
        icon: Info,
    }
]

export function AppLogo() {
    return (
        <div className="flex items-center space-x-2 px-2">
            <MessageSquareLock />
            <span className="text-lg font-semibold">Ripple</span>
        </div>
    )
}

export function AppSidebar() {
    return (
        <Sidebar variant="inset">
            <SidebarHeader>
                <AppLogo />
            </SidebarHeader>
            <SidebarMenu>
                <SidebarContent>
                    <SidebarGroup>
                        <SidebarGroupContent>
                            {items.map((item) => (
                                <SidebarMenuItem key={item.title}>
                                    <SidebarMenuButton asChild>
                                        <Link to={item.url}>
                                            <item.icon />
                                            <span>{item.title}</span>
                                        </Link>
                                    </SidebarMenuButton>
                                </SidebarMenuItem>
                            ))}
                        </SidebarGroupContent>
                    </SidebarGroup>
                </SidebarContent>
            </SidebarMenu>
        </Sidebar>
    )
}