import { GithubIcon } from "../icons";
import { CircleAlert, X } from "lucide-react";
import type { GitHubNotice as Notice } from "../githubActivity";
import "./IslandNotifications.css";
import "./IslandGithub.css";

export function GithubNotice({
	notice,
	open,
	dismiss
}: {
	notice: Notice;
	open: () => void;
	dismiss: () => void;
}) {
	return (
		<div
			className="notification-toast github-notice"
			data-priority={notice.priority}
			role="status"
			aria-live={notice.priority === "critical" ? "assertive" : "polite"}
		>
			<span className="github-notice-icon">
				{notice.priority === "critical" ? <CircleAlert size={19} /> : <GithubIcon size={20} />}
			</span>
			<button
				className="notification-toast-content"
				onClick={(event) => {
					event.stopPropagation();
					open();
				}}
				aria-label={`Abrir ${notice.repository} en el panel de GitHub`}
			>
				<strong>{notice.title}</strong>
				<span className="notification-toast-body">
					{notice.repository} · {notice.body}
				</span>
			</button>
			<time>Ahora</time>
			<button
				className="notification-toast-dismiss"
				type="button"
				onClick={(event) => {
					event.stopPropagation();
					dismiss();
				}}
				aria-label="Cerrar aviso de GitHub"
				title="Cerrar aviso"
			>
				<X size={16} />
			</button>
		</div>
	);
}
