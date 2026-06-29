// 业务职责：用紧凑链路条说明 sub2org 的核心协议路径，帮助访问者快速理解请求如何穿过 relay。
import { ChevronRight } from "lucide-react";
import type { SiteDictionary } from "@/lib/content";

type ProtocolStripProps = {
  content: SiteDictionary;
};

/**
 * 业务职责：把消费者、服务端、worker 和 SSE 输出四个业务节点串联展示，降低架构理解门槛。
 */
export function ProtocolStrip({ content }: ProtocolStripProps) {
  return (
    <section className="protocol-strip" aria-labelledby="protocol-title">
      <div className="section-heading compact">
        <p className="eyebrow">Protocol path</p>
        <h2 id="protocol-title">{content.protocol.title}</h2>
        <p>{content.protocol.intro}</p>
      </div>

      <div className="protocol-chain">
        {content.protocol.steps.map((step, index) => (
          <div className="protocol-node" key={step.label}>
            <span className="node-label">{step.label}</span>
            <strong>{step.value}</strong>
            {index < content.protocol.steps.length - 1 ? (
              <ChevronRight aria-hidden="true" className="chain-icon" size={22} />
            ) : null}
          </div>
        ))}
      </div>
    </section>
  );
}
