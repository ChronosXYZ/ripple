import { Button } from "@/components/ui/button";
import { Item, ItemActions, ItemContent, ItemDescription, ItemMedia, ItemTitle } from "@/components/ui/item";
import { Delete, Edit } from "lucide-react";
import Jdenticon from 'react-jdenticon';


const items = [
    { hash: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh', name: 'Identity 1' },
    { hash: 'bc1q59e7tgk6m623u9vmffle4yy00sh36nyluk8vuh', name: 'Identity 2' },
    { hash: 'bc1qrqlamjhy2qp0xj5mxv4sx7ra9qfmfxllf93l26', name: 'Identity 3' },
]

export function IdentitiesPage() {
    return (
        <div className="flex w-full flex-col gap-2">
            {
                items.map((item) => (
                    <Item variant="outline" key={item.hash}>
                        <ItemMedia>
                            <Jdenticon size="32" value={item.hash} />
                        </ItemMedia>
                        <ItemContent>
                            <ItemTitle>{item.name}</ItemTitle>
                            <ItemDescription>
                                {item.hash}
                            </ItemDescription>
                        </ItemContent>
                        <ItemActions>
                            <Button variant="outline" size="sm" className="mr-2">
                                <Edit />
                            </Button>
                            <Button variant="outline" size="sm">
                                < Delete />
                            </Button>
                        </ItemActions >
                    </Item >
                ))
            }
        </div >
    );
}