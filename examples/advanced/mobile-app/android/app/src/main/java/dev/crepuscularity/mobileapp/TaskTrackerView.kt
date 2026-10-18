import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Divider
import androidx.compose.material3.FilterChip
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonPrimitive

object CrepusActions {
    var dispatch: (String) -> String = { "{}" }
    var resultSink: (String) -> Unit = {}

    fun applyResult(raw: String) {
        CrepusStateStore.applyResult(raw)
    }

    fun perform(action: String) {
        val dispatch = dispatch
        val resultSink = resultSink
        Thread {
            val result = dispatch(action)
            android.os.Handler(android.os.Looper.getMainLooper()).post {
                resultSink(result)
            }
        }.start()
    }

    fun performChange(action: String?, bind: String, value: JsonElement) {
        resultSink(CrepusRustActions.dispatchChangeJson(action ?: "", bind, value.toString()))
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun TaskTrackerView(modifier: Modifier = Modifier) {
    CompositionLocalProvider(LocalContentColor provides Color(0xFFFAFAFA)) {
        Column(modifier = modifier.fillMaxSize().background(Color(0xFF0A0A0A)), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Column(modifier = Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Scaffold(bottomBar = {
                    NavigationBar {
                            NavigationBarItem(selected = CrepusStateStore.text("current_tab") == "tasks", onClick = { CrepusActions.performChange(null, "current_tab", JsonPrimitive("tasks")) }, icon = {}, label = { Text("Tasks") })
                            NavigationBarItem(selected = CrepusStateStore.text("current_tab") == "notes", onClick = { CrepusActions.performChange(null, "current_tab", JsonPrimitive("notes")) }, icon = {}, label = { Text("Notes") })
                            NavigationBarItem(selected = CrepusStateStore.text("current_tab") == "settings", onClick = { CrepusActions.performChange(null, "current_tab", JsonPrimitive("settings")) }, icon = {}, label = { Text("Settings") })
                        }
                    }, modifier = Modifier.fillMaxSize()) { innerPadding ->
                    Column(modifier = Modifier.padding(innerPadding)) {
                        if (CrepusStateStore.text("current_tab") == "tasks") {
                            Column(modifier = Modifier.verticalScroll(rememberScrollState()).fillMaxWidth().padding(horizontal = 20.dp, vertical = 16.dp)) {
                                Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                                    Row(horizontalArrangement = Arrangement.SpaceBetween) {
                                        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                                            Text("Tasks")
                                        }
                                        Button(onClick = { CrepusActions.perform("tasks.add") }, colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF22C55E), contentColor = Color(0xFF000000)), elevation = ButtonDefaults.buttonElevation(defaultElevation = 0.dp, pressedElevation = 0.dp, focusedElevation = 0.dp, hoveredElevation = 0.dp, disabledElevation = 0.dp), shape = RoundedCornerShape(8.dp), contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp)) {
                                            Text("Add")
                                        }
                                    }
                                    if (CrepusStateStore.bool("tasks_count <= 0")) {
                                        CompositionLocalProvider(LocalContentColor provides Color(0xFF71717A)) {
                                            Column(modifier = Modifier.fillMaxWidth().padding(horizontal = 0.dp, vertical = 32.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                                                Text("No tasks yet. Tap Add to get started.")
                                            }
                                        }
                                    }
                                    CrepusStateStore.items("tasks").forEachIndexed { _, task ->
                                        Row(modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp)).background(Color(0xFF18181B)).padding(horizontal = 16.dp, vertical = 12.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                                            Row() {
                                                Text("")
                                                Switch(checked = CrepusStateStore.bool("task.done", scopeName = "task", scope = task), onCheckedChange = { CrepusActions.performChange("tasks.toggle", "task.done", JsonPrimitive(it)) })
                                            }
                                            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                                                Text(CrepusStateStore.text("task.title", scopeName = "task", scope = task), fontSize = 16.0.sp)
                                                if (CrepusStateStore.bool("task.due != \"\"", scopeName = "task", scope = task)) {
                                                    Text(CrepusStateStore.text("task.due", scopeName = "task", scope = task), fontSize = 12.0.sp, color = Color(0xFF71717A))
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if (CrepusStateStore.text("current_tab") == "notes") {
                            Column(modifier = Modifier.verticalScroll(rememberScrollState()).fillMaxWidth().padding(horizontal = 20.dp, vertical = 16.dp)) {
                                Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                                    Row(horizontalArrangement = Arrangement.SpaceBetween) {
                                        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                                            Text("Notes")
                                        }
                                        Button(onClick = { CrepusActions.perform("notes.add") }, colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF3B82F6), contentColor = Color(0xFFFFFFFF)), elevation = ButtonDefaults.buttonElevation(defaultElevation = 0.dp, pressedElevation = 0.dp, focusedElevation = 0.dp, hoveredElevation = 0.dp, disabledElevation = 0.dp), shape = RoundedCornerShape(8.dp), contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp)) {
                                            Text("New")
                                        }
                                    }
                                    if (CrepusStateStore.bool("notes_count <= 0")) {
                                        CompositionLocalProvider(LocalContentColor provides Color(0xFF71717A)) {
                                            Column(modifier = Modifier.fillMaxWidth().padding(horizontal = 0.dp, vertical = 32.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                                                Text("No notes yet.")
                                            }
                                        }
                                    }
                                    CrepusStateStore.items("notes").forEachIndexed { _, note ->
                                        Column(modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp)).background(Color(0xFF18181B)).padding(horizontal = 16.dp, vertical = 12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                                            Text(CrepusStateStore.text("note.title", scopeName = "note", scope = note), fontSize = 16.0.sp, fontWeight = FontWeight.Medium)
                                            Text(CrepusStateStore.text("note.preview", scopeName = "note", scope = note), fontSize = 14.0.sp, color = Color(0xFFA1A1AA))
                                        }
                                    }
                                }
                            }
                        }
                        if (CrepusStateStore.text("current_tab") == "settings") {
                            Column(modifier = Modifier.verticalScroll(rememberScrollState()).fillMaxWidth().padding(horizontal = 20.dp, vertical = 16.dp)) {
                                Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                                        Text("Settings")
                                    }
                                    Column(modifier = Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                                        Row(modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(Color(0xFF18181B)).padding(horizontal = 16.dp, vertical = 12.dp), horizontalArrangement = Arrangement.SpaceBetween) {
                                            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                                                Text("Dark mode")
                                            }
                                            Row() {
                                                Text("")
                                                Switch(checked = CrepusStateStore.bool("dark_mode"), onCheckedChange = { CrepusActions.performChange("settings.darkMode", "dark_mode", JsonPrimitive(it)) })
                                            }
                                        }
                                        Row(modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(Color(0xFF18181B)).padding(horizontal = 16.dp, vertical = 12.dp), horizontalArrangement = Arrangement.SpaceBetween) {
                                            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                                                Text("Notifications")
                                            }
                                            Row() {
                                                Text("")
                                                Switch(checked = CrepusStateStore.bool("notifications"), onCheckedChange = { CrepusActions.performChange("settings.notifications", "notifications", JsonPrimitive(it)) })
                                            }
                                        }
                                        Row(modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(Color(0xFF18181B)).padding(horizontal = 16.dp, vertical = 12.dp), horizontalArrangement = Arrangement.SpaceBetween) {
                                            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                                                Text("Sync")
                                            }
                                            Row() {
                                                Text("")
                                                Switch(checked = CrepusStateStore.bool("sync_enabled"), onCheckedChange = { CrepusActions.performChange("settings.sync", "sync_enabled", JsonPrimitive(it)) })
                                            }
                                        }
                                        Column(modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(Color(0xFF18181B)).padding(horizontal = 16.dp, vertical = 12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                                            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                                                Text("Font size")
                                            }
                                            Column(modifier = Modifier.fillMaxWidth()) {
                                                Slider(value = CrepusStateStore.number("font_size"), onValueChange = { CrepusActions.performChange(null, "font_size", JsonPrimitive(it)) }, valueRange = 12.000f..24.000f)
                                            }
                                        }
                                    }
                                    Column(modifier = Modifier.padding(start = 0.dp, top = 16.dp, end = 0.dp, bottom = 0.dp).fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                                        Text(CrepusStateStore.text("app_version"), fontSize = 14.0.sp, color = Color(0xFF71717A))
                                        Button(onClick = { CrepusActions.perform("settings.reset") }, colors = ButtonDefaults.buttonColors(containerColor = Color(0xFFEF4444), contentColor = Color(0xFFFFFFFF)), elevation = ButtonDefaults.buttonElevation(defaultElevation = 0.dp, pressedElevation = 0.dp, focusedElevation = 0.dp, hoveredElevation = 0.dp, disabledElevation = 0.dp), shape = RoundedCornerShape(8.dp), contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp)) {
                                            Text("Reset all data")
                                        }
                                    }
                                }
                            }
                        }
                        }
                }
            }
        }
    }
}
