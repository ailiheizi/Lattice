package com.lattice.demo

import android.os.Bundle
import android.widget.*
import androidx.appcompat.app.AppCompatActivity

/**
 * Lattice Android Demo
 *
 * 演示通过 UniFFI 调用 Rust 核心：
 * - 创建身份（密钥对生成）
 * - 发送消息（本地存储）
 * - 搜索消息（Tantivy 全文检索）
 */
class MainActivity : AppCompatActivity() {

    // TODO: 当 FFI 编译完成后取消注释
    // private lateinit var client: LatticeClient

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        val txtFingerprint = findViewById<TextView>(R.id.txt_fingerprint)
        val txtLog = findViewById<TextView>(R.id.txt_log)
        val edtMessage = findViewById<EditText>(R.id.edt_message)
        val edtSearch = findViewById<EditText>(R.id.edt_search)
        val btnInit = findViewById<Button>(R.id.btn_init)
        val btnSend = findViewById<Button>(R.id.btn_send)
        val btnSearch = findViewById<Button>(R.id.btn_search)

        fun log(msg: String) {
            txtLog.append("$msg\n")
        }

        btnInit.setOnClickListener {
            try {
                val dataDir = filesDir.resolve("lattice").absolutePath
                // client = LatticeClient(dataDir)
                // val card = client.createIdentity("Android User", "ws://localhost:9100")
                // txtFingerprint.text = "Fingerprint: ${client.getFingerprint()}"
                // log("Identity created: ${card.identity.displayName}")

                // 模拟（FFI 未编译时）
                txtFingerprint.text = "Fingerprint: [FFI not compiled yet]"
                log("✓ Init called (FFI placeholder)")
            } catch (e: Exception) {
                log("✗ Error: ${e.message}")
            }
        }

        btnSend.setOnClickListener {
            val text = edtMessage.text.toString().trim()
            if (text.isEmpty()) return@setOnClickListener
            try {
                // val msgId = client.sendMessage("demo-room", text)
                // log("✓ Sent: $text (id: $msgId)")

                log("✓ Send: $text (FFI placeholder)")
                edtMessage.text.clear()
            } catch (e: Exception) {
                log("✗ Send error: ${e.message}")
            }
        }

        btnSearch.setOnClickListener {
            val query = edtSearch.text.toString().trim()
            if (query.isEmpty()) return@setOnClickListener
            try {
                // val results = client.search(query, 10u)
                // log("Search '$query': ${results.size} results")
                // results.forEach { log("  - ${it.snippet} (score: ${it.score})") }

                log("✓ Search: $query (FFI placeholder)")
            } catch (e: Exception) {
                log("✗ Search error: ${e.message}")
            }
        }
    }
}
