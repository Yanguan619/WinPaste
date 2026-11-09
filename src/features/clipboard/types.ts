import type { MouseEvent, ReactNode } from "react";
import type { DragControls } from "framer-motion";
import type { ClipboardEntry } from "../../shared/types";

export interface ClipboardItemProps {
  item: ClipboardEntry;
  index: number;
  onSelect: () => void;
  onCopy: (withFormat?: boolean) => void;
  onToggleReveal: (e: MouseEvent) => void;
  onOpen: (e: MouseEvent) => void;
  onTogglePin: (e: MouseEvent) => void;
  onDelete: (e: MouseEvent) => void;
  onToggleTagEditor: (e: MouseEvent) => void;
  onTagInput: (val: string) => void;
  onTagAdd: () => void;
  onTagDelete: (tag: string) => void;
  onStickyCreate?: (e: MouseEvent) => void;
  onExternalMissing?: (id: number) => void;
  dragControls?: DragControls;
  id?: string;
  disableLayout?: boolean;
}

export type ClipboardRenderItem<T = ClipboardEntry> = (
  item: T,
  index: number,
  isFirst: boolean
) => ReactNode;

export interface VirtualClipboardListProps<T = ClipboardEntry> {
  items: T[];
  renderItem: ClipboardRenderItem<T>;
  onLoadMore?: () => void;
  hasMore: boolean;
  isLoading: boolean;
  onScroll?: (offset: number) => void;
  compactMode: boolean;
  header?: ReactNode;
}

export interface VirtualClipboardListHandle {
  scrollToItem: (index: number, align?: 'start' | 'center' | 'end') => void;
  scrollToTop: () => void;
  resetAfterIndex: (index: number) => void;
}
