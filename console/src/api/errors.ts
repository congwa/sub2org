export type ApiErrorKind =
  | "unauthorized"
  | "insufficient_balance"
  | "forbidden"
  | "not_found"
  | "conflict"
  | "rate_limited"
  | "no_worker"
  | "bad_gateway"
  | "timeout"
  | "validation"
  | "network"
  | "unknown";

export class ApiError extends Error {
  constructor(
    public kind: ApiErrorKind,
    public status: number,
    message: string,
    public type?: string,
    public retryable = false,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

function kindFor(status: number): ApiErrorKind {
  switch (status) {
    case 401:
      return "unauthorized";
    case 402:
      return "insufficient_balance";
    case 403:
      return "forbidden";
    case 404:
      return "not_found";
    case 409:
      return "conflict";
    case 429:
      return "rate_limited";
    case 502:
      return "bad_gateway";
    case 503:
      return "no_worker";
    case 504:
      return "timeout";
    default:
      return status >= 400 && status < 500 ? "validation" : "unknown";
  }
}

export async function toApiError(res: Response): Promise<ApiError> {
  let message = res.statusText;
  let type: string | undefined;
  try {
    const body = await res.json();
    if (body?.error?.message) {
      message = body.error.message;
      type = body.error.type;
    }
  } catch {
    /* non-JSON */
  }
  const retryable = res.status === 503 || res.status === 429 || res.status === 504;
  return new ApiError(kindFor(res.status), res.status, message, type, retryable);
}
