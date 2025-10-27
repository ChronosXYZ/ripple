import { MessageSquareLock } from "lucide-react";

export function AboutPage() {
    return (
        <div className="flex items-center justify-center min-h-full p-8">
            <div className="max-w-md text-center space-y-6">
                <div>
                    <div className="flex items-center justify-center space-x-2 mb-1">
                        <MessageSquareLock />
                        <h1 className="text-3xl font-bold mb-2">Ripple</h1>
                    </div>
                    <p className="text-sm text-muted-foreground">BitMessage-inspired messaging client</p>
                </div>

                <div className="space-y-4">
                    <div>
                        <h3 className="font-semibold">Version</h3>
                        <p className="text-sm">1.0.0-beta</p>
                    </div>

                    <div>
                        <h3 className="font-semibold">Description</h3>
                        <p className="text-sm text-muted-foreground">
                            Messaging protocol and desktop client inspired by the Bitmessage project built with Rust and Tauri.
                            Send encrypted, decentralized messages without compromising your privacy.
                        </p>
                    </div>

                    <div>
                        <h3 className="font-semibold">Developer</h3>
                        <p className="text-sm">PigeonCatcher</p>
                    </div>

                    <div className="pt-4 text-xs text-muted-foreground">
                        <p>Open source software licensed under MIT</p>
                    </div>
                </div>
            </div>
        </div>
    );
}