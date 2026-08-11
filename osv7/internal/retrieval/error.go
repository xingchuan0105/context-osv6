package retrieval

import "encoding/json"

// ErrorBody is the L0 agent-actionable error shape (third-person facts).
type ErrorBody struct {
	Error       string `json:"error"`
	Capability  string `json:"capability,omitempty"`
	Fact        string `json:"fact"`
	Remediation string `json:"remediation,omitempty"`
	Detail      any    `json:"detail,omitempty"`
}

func (e ErrorBody) JSON() string {
	b, _ := json.MarshalIndent(e, "", "  ")
	return string(b)
}

func errCardMissing() ErrorBody {
	return ErrorBody{
		Error:       "query_card_missing",
		Fact:        "本检索任务尚未提交题卡；无卡不检索。",
		Remediation: "先调用 set_query_card，声明 workspace_id 与 required_actions 后再调用检索原语。",
	}
}

func errResource(fact, remediation string, detail any) ErrorBody {
	return ErrorBody{
		Error:       "resource_gate",
		Fact:        fact,
		Remediation: remediation,
		Detail:      detail,
	}
}

func errContract(fact string, detail any) ErrorBody {
	return ErrorBody{
		Error:       "contract_gate",
		Fact:        fact,
		Remediation: "题卡声明的 harness 可见动作须有对应 Ok 回传；可改卡撤销声明后重试。",
		Detail:      detail,
	}
}

func errCapability(cap, fact, remediation string) ErrorBody {
	return ErrorBody{
		Error:       "capability_missing",
		Capability:  cap,
		Fact:        fact,
		Remediation: remediation,
	}
}
