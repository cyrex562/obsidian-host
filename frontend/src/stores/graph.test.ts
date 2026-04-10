import { describe, it, expect, vi, beforeEach } from 'vitest';
import { setActivePinia, createPinia } from 'pinia';
import { useGraphStore } from './graph';

// Mock API modules
vi.mock('@/api/client', () => ({
    apiGetGraph: vi.fn(),
    apiListEntityTypes: vi.fn(),
}));

import { apiGetGraph, apiListEntityTypes } from '@/api/client';

const mockNodes = [
    { id: 'id1', path: 'characters/alice.md', entity_type: 'character', labels: ['graphable'], title: 'Alice' },
    { id: 'id2', path: 'locations/city.md', entity_type: 'location', labels: ['graphable', 'place'], title: 'The City' },
    { id: 'id3', path: 'factions/guild.md', entity_type: 'faction', labels: ['graphable'], title: 'The Guild' },
];

const mockEdges = [
    { id: 'e1', source: 'id1', target: 'id2', label: 'located_in', relation_type: 'located_in', source_field: 'location', direction: 'outgoing', is_inverse: false },
    { id: 'e2', source: 'id1', target: 'id3', label: 'member_of', relation_type: 'member_of', source_field: 'faction', direction: 'outgoing', is_inverse: false },
];

const mockEntityTypes = [
    { id: 'character', plugin_id: 'worldbuilding', name: 'Character', color: '#4A90D9', icon: 'person', labels: [], fields: [], show_on_create: [] },
    { id: 'location', plugin_id: 'worldbuilding', name: 'Location', color: '#7ED321', icon: 'location_on', labels: [], fields: [], show_on_create: [] },
    { id: 'faction', plugin_id: 'worldbuilding', name: 'Faction', color: '#D0021B', icon: 'group', labels: [], fields: [], show_on_create: [] },
];

describe('useGraphStore', () => {
    beforeEach(() => {
        setActivePinia(createPinia());
        vi.clearAllMocks();
    });

    // ── loadGraph ─────────────────────────────────────────────────────────

    it('starts with empty state', () => {
        const store = useGraphStore();
        expect(store.nodes).toEqual([]);
        expect(store.edges).toEqual([]);
        expect(store.loading).toBe(false);
        expect(store.error).toBeNull();
    });

    it('loads graph data and entity types', async () => {
        vi.mocked(apiGetGraph).mockResolvedValueOnce({ nodes: mockNodes, edges: mockEdges });
        vi.mocked(apiListEntityTypes).mockResolvedValueOnce({ entity_types: mockEntityTypes });

        const store = useGraphStore();
        await store.loadGraph('vault-1');

        expect(store.nodes.length).toBe(3);
        expect(store.edges.length).toBe(2);
        expect(store.entityTypes.length).toBe(3);
        expect(store.loadedVaultId).toBe('vault-1');
        expect(store.loading).toBe(false);
        expect(store.error).toBeNull();
    });

    it('enriches node colors from entity type schema', async () => {
        vi.mocked(apiGetGraph).mockResolvedValueOnce({ nodes: mockNodes, edges: mockEdges });
        vi.mocked(apiListEntityTypes).mockResolvedValueOnce({ entity_types: mockEntityTypes });

        const store = useGraphStore();
        await store.loadGraph('vault-1');

        const alice = store.nodes.find((n) => n.id === 'id1');
        expect(alice?.color).toBe('#4A90D9'); // character color
        const city = store.nodes.find((n) => n.id === 'id2');
        expect(city?.color).toBe('#7ED321'); // location color
    });

    it('does not reload when same vault already loaded', async () => {
        vi.mocked(apiGetGraph).mockResolvedValue({ nodes: mockNodes, edges: mockEdges });
        vi.mocked(apiListEntityTypes).mockResolvedValue({ entity_types: mockEntityTypes });

        const store = useGraphStore();
        await store.loadGraph('vault-1');
        await store.loadGraph('vault-1'); // second call should be no-op

        expect(apiGetGraph).toHaveBeenCalledTimes(1);
    });

    it('force-reloads when force=true', async () => {
        vi.mocked(apiGetGraph).mockResolvedValue({ nodes: mockNodes, edges: mockEdges });
        vi.mocked(apiListEntityTypes).mockResolvedValue({ entity_types: mockEntityTypes });

        const store = useGraphStore();
        await store.loadGraph('vault-1');
        await store.loadGraph('vault-1', true); // force

        expect(apiGetGraph).toHaveBeenCalledTimes(2);
    });

    it('sets error state on API failure', async () => {
        vi.mocked(apiGetGraph).mockRejectedValueOnce(new Error('Network error'));
        vi.mocked(apiListEntityTypes).mockResolvedValueOnce({ entity_types: [] });

        const store = useGraphStore();
        await store.loadGraph('vault-1');

        expect(store.error).toBe('Network error');
        expect(store.loading).toBe(false);
    });

    // ── filteredNodes ─────────────────────────────────────────────────────

    it('filteredNodes returns all nodes when all types visible', async () => {
        vi.mocked(apiGetGraph).mockResolvedValueOnce({ nodes: mockNodes, edges: mockEdges });
        vi.mocked(apiListEntityTypes).mockResolvedValueOnce({ entity_types: mockEntityTypes });

        const store = useGraphStore();
        await store.loadGraph('vault-1');

        expect(store.filteredNodes.length).toBe(3);
    });

    it('filteredNodes filters by type when setTypeFilter called', async () => {
        vi.mocked(apiGetGraph).mockResolvedValueOnce({ nodes: mockNodes, edges: mockEdges });
        vi.mocked(apiListEntityTypes).mockResolvedValueOnce({ entity_types: mockEntityTypes });

        const store = useGraphStore();
        await store.loadGraph('vault-1');
        store.setTypeFilter(['character']);

        const filtered = store.filteredNodes;
        expect(filtered.length).toBe(1);
        expect(filtered[0].id).toBe('id1');
    });

    it('filteredNodes filters by search query', async () => {
        vi.mocked(apiGetGraph).mockResolvedValueOnce({ nodes: mockNodes, edges: mockEdges });
        vi.mocked(apiListEntityTypes).mockResolvedValueOnce({ entity_types: mockEntityTypes });

        const store = useGraphStore();
        await store.loadGraph('vault-1');
        store.searchQuery = 'alice';

        const filtered = store.filteredNodes;
        expect(filtered.length).toBe(1);
        expect(filtered[0].title).toBe('Alice');
    });

    it('filteredNodes search is case-insensitive', async () => {
        vi.mocked(apiGetGraph).mockResolvedValueOnce({ nodes: mockNodes, edges: mockEdges });
        vi.mocked(apiListEntityTypes).mockResolvedValueOnce({ entity_types: mockEntityTypes });

        const store = useGraphStore();
        await store.loadGraph('vault-1');
        store.searchQuery = 'GUILD';

        expect(store.filteredNodes.length).toBe(1);
        expect(store.filteredNodes[0].title).toBe('The Guild');
    });

    // ── filteredEdges ─────────────────────────────────────────────────────

    it('filteredEdges removes edges whose endpoints are filtered out', async () => {
        vi.mocked(apiGetGraph).mockResolvedValueOnce({ nodes: mockNodes, edges: mockEdges });
        vi.mocked(apiListEntityTypes).mockResolvedValueOnce({ entity_types: mockEntityTypes });

        const store = useGraphStore();
        await store.loadGraph('vault-1');
        // Only show location nodes
        store.setTypeFilter(['location']);

        // Both edges start from id1 (character) which is filtered out
        expect(store.filteredEdges.length).toBe(0);
    });

    it('filteredEdges keeps edges where both endpoints pass filter', async () => {
        vi.mocked(apiGetGraph).mockResolvedValueOnce({ nodes: mockNodes, edges: mockEdges });
        vi.mocked(apiListEntityTypes).mockResolvedValueOnce({ entity_types: mockEntityTypes });

        const store = useGraphStore();
        await store.loadGraph('vault-1');
        store.setTypeFilter(['character', 'location']);

        // e1 connects character → location: should be kept
        // e2 connects character → faction: faction is filtered, so dropped
        expect(store.filteredEdges.length).toBe(1);
        expect(store.filteredEdges[0].id).toBe('e1');
    });

    // ── toggleType ────────────────────────────────────────────────────────

    it('toggleType adds a type when absent', async () => {
        vi.mocked(apiGetGraph).mockResolvedValueOnce({ nodes: mockNodes, edges: mockEdges });
        vi.mocked(apiListEntityTypes).mockResolvedValueOnce({ entity_types: mockEntityTypes });

        const store = useGraphStore();
        await store.loadGraph('vault-1');
        store.setTypeFilter([]); // clear all
        store.toggleType('character');

        expect(store.visibleTypeIds.has('character')).toBe(true);
    });

    it('toggleType removes a type when present', async () => {
        vi.mocked(apiGetGraph).mockResolvedValueOnce({ nodes: mockNodes, edges: mockEdges });
        vi.mocked(apiListEntityTypes).mockResolvedValueOnce({ entity_types: mockEntityTypes });

        const store = useGraphStore();
        await store.loadGraph('vault-1');
        // All types visible; toggle off character
        store.toggleType('character');

        expect(store.visibleTypeIds.has('character')).toBe(false);
    });

    // ── availableTypes ────────────────────────────────────────────────────

    it('availableTypes only includes types that have nodes', async () => {
        const partialNodes = [mockNodes[0]]; // only alice (character)
        vi.mocked(apiGetGraph).mockResolvedValueOnce({ nodes: partialNodes, edges: [] });
        vi.mocked(apiListEntityTypes).mockResolvedValueOnce({ entity_types: mockEntityTypes });

        const store = useGraphStore();
        await store.loadGraph('vault-1');

        expect(store.availableTypes.length).toBe(1);
        expect(store.availableTypes[0].id).toBe('character');
    });

    // ── invalidate ────────────────────────────────────────────────────────

    it('invalidate clears loadedVaultId so next loadGraph re-fetches', async () => {
        vi.mocked(apiGetGraph).mockResolvedValue({ nodes: mockNodes, edges: mockEdges });
        vi.mocked(apiListEntityTypes).mockResolvedValue({ entity_types: mockEntityTypes });

        const store = useGraphStore();
        await store.loadGraph('vault-1');
        store.invalidate();
        await store.loadGraph('vault-1');

        expect(apiGetGraph).toHaveBeenCalledTimes(2);
    });
});
