import { IconCompass, IconMovie, IconNetwork } from "@tabler/icons-react";

export type SpaceId = "library" | "discover" | "knowledge";

// Use the concrete type of any Tabler icon — they all share the same
// ForwardRefExoticComponent<IconProps & RefAttributes<SVGSVGElement>> shape.
type TablerIcon = typeof IconMovie;

export interface SpaceDef {
  id: SpaceId;
  label: string;
  /** Top-level route this space owns. */
  route: string;
  /** Tabler icon component. */
  Icon: TablerIcon;
  /** Cmd+N keyboard shortcut digit. */
  shortcutDigit: 1 | 2 | 3;
}

export const SPACES: SpaceDef[] = [
  {
    id: "library",
    label: "影片库",
    route: "/library",
    Icon: IconMovie,
    shortcutDigit: 1,
  },
  {
    id: "discover",
    label: "发现",
    route: "/discover",
    Icon: IconCompass,
    shortcutDigit: 2,
  },
  {
    id: "knowledge",
    label: "知识库",
    route: "/knowledge",
    Icon: IconNetwork,
    shortcutDigit: 3,
  },
];

/** Resolve which space a pathname belongs to (route prefix match). */
export function activeSpaceFor(pathname: string): SpaceDef | null {
  return SPACES.find((s) => pathname === s.route || pathname.startsWith(s.route + "/")) ?? null;
}
