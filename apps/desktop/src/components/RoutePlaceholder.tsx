type RoutePlaceholderProps = {
  eyebrow: string;
  title: string;
  description: string;
};

export function RoutePlaceholder({
  eyebrow,
  title,
  description,
}: RoutePlaceholderProps) {
  return (
    <section className="route-panel" aria-labelledby={`${eyebrow}-title`}>
      <p className="eyebrow">{eyebrow}</p>
      <h1 id={`${eyebrow}-title`}>{title}</h1>
      <p className="route-description">{description}</p>
      <div className="empty-state" role="status">
        <span className="empty-state__mark" aria-hidden="true">✦</span>
        <div>
          <strong>产品底座已就绪</strong>
          <p>本页业务能力将在对应 P17 后续票中接入同一个 Core。</p>
        </div>
      </div>
    </section>
  );
}
