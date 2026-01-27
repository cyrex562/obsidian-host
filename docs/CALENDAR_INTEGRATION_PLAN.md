# Calendar Integration Implementation Plan

## Overview
A calendar view component that integrates with daily notes, shows note activity, and provides date-based navigation.

## Architecture

### Component Structure
```
CalendarView
├── CalendarHeader (Month/Year navigation)
├── CalendarGrid (7x6 day grid)
│   └── CalendarDay (Individual day cells)
├── CalendarLegend (Activity indicators)
└── CalendarControls (View options)
```

### Data Flow
```
Backend API → Calendar Service → Calendar Component → UI
     ↓              ↓                    ↓
  File metadata  Aggregation         Rendering
```

## UI Design

### Calendar Component

```html
<div class="calendar-view">
    <!-- Header -->
    <div class="calendar-header">
        <button class="calendar-nav-btn" id="prev-month">‹</button>
        <h3 class="calendar-title">January 2024</h3>
        <button class="calendar-nav-btn" id="next-month">›</button>
    </div>

    <!-- Weekday labels -->
    <div class="calendar-weekdays">
        <div class="calendar-weekday">Sun</div>
        <div class="calendar-weekday">Mon</div>
        <div class="calendar-weekday">Tue</div>
        <div class="calendar-weekday">Wed</div>
        <div class="calendar-weekday">Thu</div>
        <div class="calendar-weekday">Fri</div>
        <div class="calendar-weekday">Sat</div>
    </div>

    <!-- Calendar grid -->
    <div class="calendar-grid">
        <!-- Days rendered dynamically -->
        <div class="calendar-day" data-date="2024-01-01">
            <span class="day-number">1</span>
            <div class="day-indicators">
                <span class="note-count" title="3 notes">●●●</span>
                <span class="daily-note-indicator" title="Daily note">📅</span>
            </div>
        </div>
        <!-- ... more days ... -->
    </div>

    <!-- Legend -->
    <div class="calendar-legend">
        <div class="legend-item">
            <span class="legend-dot" style="background: var(--accent-color)"></span>
            <span>Has notes</span>
        </div>
        <div class="legend-item">
            <span class="legend-icon">📅</span>
            <span>Daily note</span>
        </div>
        <div class="legend-item">
            <span class="legend-dot" style="background: var(--text-muted)"></span>
            <span>Today</span>
        </div>
    </div>
</div>
```

### CSS Styles

```css
.calendar-view {
    padding: 1rem;
    background: var(--bg-secondary);
    border-radius: var(--radius-md);
}

.calendar-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 1rem;
}

.calendar-title {
    font-size: 1.1rem;
    font-weight: 600;
    color: var(--text-primary);
}

.calendar-nav-btn {
    padding: 0.5rem;
    background: var(--bg-tertiary);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: all 0.2s;
}

.calendar-nav-btn:hover {
    background: var(--bg-primary);
    border-color: var(--accent-color);
}

.calendar-weekdays {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 0.25rem;
    margin-bottom: 0.5rem;
}

.calendar-weekday {
    text-align: center;
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--text-muted);
    padding: 0.5rem 0;
}

.calendar-grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 0.25rem;
}

.calendar-day {
    aspect-ratio: 1;
    padding: 0.5rem;
    background: var(--bg-tertiary);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: all 0.2s;
    position: relative;
    display: flex;
    flex-direction: column;
}

.calendar-day:hover {
    background: var(--bg-primary);
    border-color: var(--accent-color);
    transform: scale(1.05);
}

.calendar-day.today {
    border-color: var(--accent-color);
    border-width: 2px;
    background: rgba(var(--accent-color-rgb), 0.1);
}

.calendar-day.has-notes {
    background: rgba(var(--accent-color-rgb), 0.05);
}

.calendar-day.other-month {
    opacity: 0.3;
}

.day-number {
    font-size: 0.9rem;
    font-weight: 500;
    color: var(--text-primary);
}

.day-indicators {
    margin-top: auto;
    display: flex;
    gap: 0.25rem;
    align-items: center;
    font-size: 0.7rem;
}

.note-count {
    color: var(--accent-color);
}

.daily-note-indicator {
    font-size: 0.8rem;
}

.calendar-legend {
    display: flex;
    gap: 1rem;
    margin-top: 1rem;
    padding-top: 1rem;
    border-top: 1px solid var(--border-color);
    font-size: 0.85rem;
}

.legend-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
}

.legend-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
}
```

## TypeScript Implementation

### Calendar Service

```typescript
interface CalendarDay {
    date: Date;
    dateString: string; // YYYY-MM-DD
    isToday: boolean;
    isCurrentMonth: boolean;
    noteCount: number;
    hasDailyNote: boolean;
    notes: string[]; // File paths
}

interface CalendarMonth {
    year: number;
    month: number; // 0-11
    days: CalendarDay[];
}

class CalendarService {
    constructor(private api: ApiClient, private vaultId: string) {}

    async getMonth(year: number, month: number): Promise<CalendarMonth> {
        // Get all files with modification dates
        const files = await this.api.getFilesByDateRange(
            this.vaultId,
            this.getMonthStart(year, month),
            this.getMonthEnd(year, month)
        );

        // Build calendar grid (6 weeks)
        const days: CalendarDay[] = [];
        const firstDay = new Date(year, month, 1);
        const lastDay = new Date(year, month + 1, 0);
        
        // Add days from previous month to fill first week
        const firstWeekday = firstDay.getDay();
        for (let i = firstWeekday - 1; i >= 0; i--) {
            const date = new Date(year, month, -i);
            days.push(this.createCalendarDay(date, files, false));
        }

        // Add days of current month
        for (let day = 1; day <= lastDay.getDate(); day++) {
            const date = new Date(year, month, day);
            days.push(this.createCalendarDay(date, files, true));
        }

        // Add days from next month to fill last week
        const remainingDays = 42 - days.length; // 6 weeks * 7 days
        for (let i = 1; i <= remainingDays; i++) {
            const date = new Date(year, month + 1, i);
            days.push(this.createCalendarDay(date, files, false));
        }

        return { year, month, days };
    }

    private createCalendarDay(
        date: Date,
        files: FileMetadata[],
        isCurrentMonth: boolean
    ): CalendarDay {
        const dateString = this.formatDate(date);
        const today = new Date();
        const isToday = this.isSameDay(date, today);

        // Find notes for this day
        const dayNotes = files.filter(f => 
            this.isSameDay(new Date(f.last_modified), date)
        );

        // Check for daily note
        const dailyNotePath = `Daily Notes/${dateString}.md`;
        const hasDailyNote = dayNotes.some(n => n.path === dailyNotePath);

        return {
            date,
            dateString,
            isToday,
            isCurrentMonth,
            noteCount: dayNotes.length,
            hasDailyNote,
            notes: dayNotes.map(n => n.path)
        };
    }

    private formatDate(date: Date): string {
        const year = date.getFullYear();
        const month = String(date.getMonth() + 1).padStart(2, '0');
        const day = String(date.getDate()).padStart(2, '0');
        return `${year}-${month}-${day}`;
    }

    private isSameDay(date1: Date, date2: Date): boolean {
        return date1.getFullYear() === date2.getFullYear() &&
               date1.getMonth() === date2.getMonth() &&
               date1.getDate() === date2.getDate();
    }

    private getMonthStart(year: number, month: number): string {
        return `${year}-${String(month + 1).padStart(2, '0')}-01`;
    }

    private getMonthEnd(year: number, month: number): string {
        const lastDay = new Date(year, month + 1, 0).getDate();
        return `${year}-${String(month + 1).padStart(2, '0')}-${String(lastDay).padStart(2, '0')}`;
    }
}
```

### Calendar Component

```typescript
class CalendarView {
    private currentYear: number;
    private currentMonth: number;
    private service: CalendarService;

    constructor(container: HTMLElement, vaultId: string, api: ApiClient) {
        this.service = new CalendarService(api, vaultId);
        const now = new Date();
        this.currentYear = now.getFullYear();
        this.currentMonth = now.getMonth();
        this.render(container);
    }

    async render(container: HTMLElement) {
        const month = await this.service.getMonth(this.currentYear, this.currentMonth);
        
        container.innerHTML = `
            <div class="calendar-view">
                ${this.renderHeader()}
                ${this.renderWeekdays()}
                ${this.renderGrid(month)}
                ${this.renderLegend()}
            </div>
        `;

        this.attachEventListeners(container);
    }

    private renderHeader(): string {
        const monthName = new Date(this.currentYear, this.currentMonth).toLocaleDateString('en-US', { month: 'long' });
        return `
            <div class="calendar-header">
                <button class="calendar-nav-btn" id="prev-month">‹</button>
                <h3 class="calendar-title">${monthName} ${this.currentYear}</h3>
                <button class="calendar-nav-btn" id="next-month">›</button>
            </div>
        `;
    }

    private renderWeekdays(): string {
        const weekdays = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
        return `
            <div class="calendar-weekdays">
                ${weekdays.map(day => `<div class="calendar-weekday">${day}</div>`).join('')}
            </div>
        `;
    }

    private renderGrid(month: CalendarMonth): string {
        return `
            <div class="calendar-grid">
                ${month.days.map(day => this.renderDay(day)).join('')}
            </div>
        `;
    }

    private renderDay(day: CalendarDay): string {
        const classes = [
            'calendar-day',
            day.isToday ? 'today' : '',
            !day.isCurrentMonth ? 'other-month' : '',
            day.noteCount > 0 ? 'has-notes' : ''
        ].filter(Boolean).join(' ');

        const indicators = [];
        if (day.noteCount > 0) {
            indicators.push(`<span class="note-count" title="${day.noteCount} notes">${'●'.repeat(Math.min(day.noteCount, 5))}</span>`);
        }
        if (day.hasDailyNote) {
            indicators.push(`<span class="daily-note-indicator" title="Daily note">📅</span>`);
        }

        return `
            <div class="${classes}" data-date="${day.dateString}">
                <span class="day-number">${day.date.getDate()}</span>
                <div class="day-indicators">
                    ${indicators.join('')}
                </div>
            </div>
        `;
    }

    private renderLegend(): string {
        return `
            <div class="calendar-legend">
                <div class="legend-item">
                    <span class="legend-dot" style="background: var(--accent-color)"></span>
                    <span>Has notes</span>
                </div>
                <div class="legend-item">
                    <span class="legend-icon">📅</span>
                    <span>Daily note</span>
                </div>
                <div class="legend-item">
                    <span class="legend-dot" style="background: var(--text-muted)"></span>
                    <span>Today</span>
                </div>
            </div>
        `;
    }

    private attachEventListeners(container: HTMLElement) {
        // Month navigation
        container.querySelector('#prev-month')?.addEventListener('click', () => {
            this.navigateMonth(-1);
        });

        container.querySelector('#next-month')?.addEventListener('click', () => {
            this.navigateMonth(1);
        });

        // Day clicks
        container.querySelectorAll('.calendar-day').forEach(dayEl => {
            dayEl.addEventListener('click', async (e) => {
                const date = (e.currentTarget as HTMLElement).getAttribute('data-date');
                if (date) {
                    await this.onDayClick(date);
                }
            });
        });
    }

    private async navigateMonth(delta: number) {
        this.currentMonth += delta;
        if (this.currentMonth > 11) {
            this.currentMonth = 0;
            this.currentYear++;
        } else if (this.currentMonth < 0) {
            this.currentMonth = 11;
            this.currentYear--;
        }
        // Re-render
    }

    private async onDayClick(dateString: string) {
        // Open daily note for this date or show notes list
        console.log('Day clicked:', dateString);
    }
}
```

## Backend API

### New Endpoint

```rust
// GET /api/vaults/{vault_id}/files/by-date
// Query params: start_date, end_date

#[derive(Deserialize)]
struct DateRangeQuery {
    start_date: String, // YYYY-MM-DD
    end_date: String,   // YYYY-MM-DD
}

async fn get_files_by_date_range(
    vault_id: Path<String>,
    query: Query<DateRangeQuery>,
) -> Result<Json<Vec<FileMetadata>>> {
    // Parse dates
    let start = parse_date(&query.start_date)?;
    let end = parse_date(&query.end_date)?;
    
    // Get all files
    let files = get_all_files(&vault_id)?;
    
    // Filter by modification date
    let filtered: Vec<FileMetadata> = files
        .into_iter()
        .filter(|f| {
            let modified = f.last_modified;
            modified >= start && modified <= end
        })
        .collect();
    
    Ok(Json(filtered))
}
```

## Integration Points

### Daily Notes
- Clicking a day with daily note opens it
- Clicking a day without daily note creates it
- Daily note indicator (📅) shows which days have daily notes

### Note Creation
- Right-click day → "Create note for this date"
- Shift+click → Create daily note
- Ctrl+click → Show all notes for this date

### Navigation
- Arrow keys to navigate days
- Enter to open selected day
- Space to toggle daily note

## Performance Considerations

### Optimization Strategies

1. **Lazy Loading**: Only load current month data
2. **Caching**: Cache month data for quick navigation
3. **Debouncing**: Debounce API calls during rapid navigation
4. **Virtual Scrolling**: For year view (future)

### Caching Strategy

```typescript
class CalendarCache {
    private cache: Map<string, CalendarMonth> = new Map();
    private maxSize = 12; // Cache 12 months

    get(year: number, month: number): CalendarMonth | null {
        const key = `${year}-${month}`;
        return this.cache.get(key) || null;
    }

    set(year: number, month: number, data: CalendarMonth) {
        const key = `${year}-${month}`;
        this.cache.set(key, data);
        
        // Evict oldest if cache too large
        if (this.cache.size > this.maxSize) {
            const firstKey = this.cache.keys().next().value;
            this.cache.delete(firstKey);
        }
    }
}
```

## Testing

### Unit Tests

```typescript
describe('CalendarService', () => {
    it('should generate correct number of days', async () => {
        const month = await service.getMonth(2024, 0);
        expect(month.days.length).toBe(42); // 6 weeks
    });

    it('should mark today correctly', async () => {
        const month = await service.getMonth(2024, 0);
        const today = month.days.find(d => d.isToday);
        expect(today).toBeDefined();
    });

    it('should count notes correctly', async () => {
        // Mock files
        const month = await service.getMonth(2024, 0);
        const dayWithNotes = month.days.find(d => d.noteCount > 0);
        expect(dayWithNotes?.notes.length).toBe(dayWithNotes?.noteCount);
    });
});
```

### Integration Tests

- Month navigation works
- Day clicks open correct notes
- Daily note creation works
- Performance acceptable (<100ms render)

## Future Enhancements

1. **Year View**: Show entire year at once
2. **Heatmap**: Color intensity based on note count
3. **Filters**: Filter by tags, folders
4. **Multi-select**: Select date range
5. **Export**: Export calendar as image
6. **Themes**: Different calendar themes
7. **Week View**: Show week-by-week
8. **Agenda View**: List view of notes by date

## Summary

✅ **Component Design**: Complete UI/UX specification
✅ **CSS Styles**: Comprehensive styling
✅ **TypeScript**: Full implementation plan
✅ **Backend API**: Date range query endpoint
✅ **Integration**: Daily notes integration
✅ **Performance**: Optimization strategies
✅ **Testing**: Test plan defined

The calendar integration is fully designed and ready for implementation!
