interface CompactSignalOverlayProps {
  title: string
  summary: string
  tone?: 'info' | 'success' | 'warning' | 'danger'
  pills: ReadonlyArray<{ label: string, value: string, tone?: 'info' | 'success' | 'warning' | 'danger' }>
  actions?: ReadonlyArray<string>
}

function badgeToneClass(tone: CompactSignalOverlayProps['tone']) {
  switch (tone) {
    case 'success':
      return 'badge-success'
    case 'warning':
      return 'badge-warning'
    case 'danger':
      return 'badge-danger'
    default:
      return 'badge-info'
  }
}

export function CompactSignalOverlay({ title, summary, tone = 'info', pills, actions = [] }: CompactSignalOverlayProps) {
  return (
    <div className="sticky top-4 z-20 compact-overlay">
      <div className="compact-overlay__glow" />
      <div className="compact-overlay__body">
        <div className="flex flex-col gap-3 xl:flex-row xl:items-start xl:justify-between">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2 mb-2">
              <span className={`badge ${badgeToneClass(tone)}`}>snapshot brief</span>
              <p className="text-sm font-semibold">{title}</p>
            </div>
            <p className="text-sm leading-6" style={{ color: 'var(--text-secondary)' }}>{summary}</p>
          </div>

          <div className="flex flex-wrap gap-2 xl:justify-end">
            {pills.map((pill) => (
              <div key={`${pill.label}-${pill.value}`} className="compact-overlay__pill">
                <span className="text-[11px] uppercase tracking-[0.18em]" style={{ color: 'var(--text-muted)' }}>{pill.label}</span>
                <span className={`badge ${badgeToneClass(pill.tone ?? tone)}`}>{pill.value}</span>
              </div>
            ))}
          </div>
        </div>

        {actions.length > 0 ? (
          <div className="mt-3 flex flex-col gap-2 xl:flex-row xl:flex-wrap">
            {actions.slice(0, 2).map((action, index) => (
              <div key={`${index}-${action}`} className="compact-overlay__action">
                <span className="badge badge-info">next {index + 1}</span>
                <p className="text-xs leading-5" style={{ color: 'var(--text-secondary)' }}>{action}</p>
              </div>
            ))}
          </div>
        ) : null}
      </div>
    </div>
  )
}
