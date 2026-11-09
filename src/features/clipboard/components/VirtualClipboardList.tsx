import React, { useRef, useImperativeHandle, useCallback, useMemo } from 'react';
import { Virtuoso, VirtuosoHandle } from 'react-virtuoso';
import type { ListRange } from 'react-virtuoso';
import type { ClipboardEntry } from "../../../shared/types";
import type { VirtualClipboardListHandle, VirtualClipboardListProps } from "../types";

type VirtuosoListContext = {
    header?: React.ReactNode;
    hasMore: boolean;
    isLoading: boolean;
};

const ListHeader = ({ context }: { context?: VirtuosoListContext }) => {
    const header = context?.header;
    return header ? <div className="list-header">{header}</div> : null;
};

const ListFooter = ({ context }: { context?: VirtuosoListContext }) => {
    if (!context) return null;
    const { isLoading, hasMore } = context;
    if (!isLoading && !hasMore) return null;

    return (
        <div style={{
            padding: '20px',
            textAlign: 'center',
            opacity: 0.6,
            fontSize: '12px',
            color: 'var(--text-secondary)'
        }}>
            {isLoading ? '加载中...' : '加载更多...'}
        </div>
    );
};

/**
 * 通用虚拟列表组件，支持任意数据类型。
 * 保留对 ClipboardEntry 的兼容（默认泛型参数）。
 */
function VirtualList<T = ClipboardEntry>(
    props: VirtualClipboardListProps<T> & {
        listRef?: React.RefObject<VirtualClipboardListHandle | null>;
    }
) {
    const {
        items,
        renderItem,
        onLoadMore,
        hasMore,
        isLoading,
        selectedIndex,
        isKeyboardMode,
        onScroll,
        compactMode,
        header,
        listRef
    } = props;

    const virtuosoRef = useRef<VirtuosoHandle>(null);
    const visibleRangeRef = useRef<ListRange | null>(null);
    useImperativeHandle(listRef, () => ({
        scrollToItem: (index: number) => {
            virtuosoRef.current?.scrollIntoView({
                index,
                behavior: 'smooth',
                align: 'center',
            });
        },
        scrollToTop: () => {
            virtuosoRef.current?.scrollTo({
                top: 0,
                behavior: 'auto'
            });
        },
        resetAfterIndex: (_index: number) => {
            // Not needed with Virtuoso as it handles dynamic heights automatically
        }
    }));

    // Keep keyboard selection visible even when the item is only in overscan
    React.useEffect(() => {
        if (!isKeyboardMode) return;

        if (selectedIndex < 0) {
            virtuosoRef.current?.scrollTo({ top: 0, behavior: 'smooth' });
            return;
        }

        const range = visibleRangeRef.current;
        const edgeBuffer = 1;

        if (!range) {
            virtuosoRef.current?.scrollToIndex({
                index: selectedIndex,
                behavior: 'auto',
                align: 'center',
            });
            return;
        }

        if (selectedIndex < range.startIndex + edgeBuffer) {
            virtuosoRef.current?.scrollToIndex({
                index: selectedIndex,
                behavior: 'auto',
                align: 'start',
            });
            return;
        }

        if (selectedIndex > range.endIndex - edgeBuffer) {
            virtuosoRef.current?.scrollToIndex({
                index: selectedIndex,
                behavior: 'auto',
                align: 'end',
            });
        }
    }, [selectedIndex, isKeyboardMode]);


    // Handle scroll events
    const handleScroll = useCallback((scrollTop: number) => {
        onScroll?.(scrollTop);
    }, [onScroll]);

    // Handle end reached for infinite loading
    const handleEndReached = useCallback(() => {
        if (hasMore && !isLoading && onLoadMore) {
            onLoadMore();
        }
    }, [hasMore, isLoading, onLoadMore]);

    const handleRangeChanged = useCallback((range: ListRange) => {
        visibleRangeRef.current = range;
    }, []);

    // Memoized item renderer for Virtuoso
    const itemContent = useCallback((index: number, item: T) => {
        return (
            <div style={{
                paddingBottom: compactMode ? 0 : 4,
            }}>
                {renderItem(item, index, index === 0)}
            </div>
        );
    }, [renderItem, compactMode]);

    const components = useMemo(() => ({
        Header: ListHeader,
        Footer: ListFooter
    }), []);

    const context = useMemo(() => ({
        header,
        hasMore,
        isLoading
    }), [header, hasMore, isLoading]);

    return (
        <div className="virtual-list-wrapper" style={{ height: '100%', width: '100%' }}>
            <Virtuoso<T>
                ref={virtuosoRef}
                data={items}
                itemContent={itemContent}
                components={components}
                context={context}
                style={{ height: '100%' }}
                onScroll={(e) => handleScroll((e.currentTarget as HTMLElement).scrollTop)}
                endReached={handleEndReached}
                rangeChanged={handleRangeChanged}
                overscan={200}
            />
        </div>
    );
}

VirtualList.displayName = 'VirtualList';

export { VirtualList };
export default VirtualList;

// 向后兼容：保持 ClipboardEntry 专用的导出
export type ClipboardRenderItem = (
    item: ClipboardEntry,
    index: number,
    isFirst: boolean
) => React.ReactNode;
