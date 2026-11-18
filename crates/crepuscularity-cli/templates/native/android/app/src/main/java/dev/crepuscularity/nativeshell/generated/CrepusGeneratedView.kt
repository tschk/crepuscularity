package dev.crepuscularity.nativeshell

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Divider
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

object CrepusActions {
    var dispatch: (String) -> String = { "{}" }
    var resultSink: (String) -> Unit = {}

    fun perform(action: String) {
        resultSink(dispatch(action))
    }
}

@Composable
fun CrepusGeneratedView(modifier: Modifier = Modifier) {
    Column(modifier = modifier.fillMaxSize().background(Color(0xFF101624)).padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text("Crepus Mobile", fontSize = 12.0.sp, color = Color(0xFFC7D2FE))
            Text("Launch Control", fontSize = 24.0.sp, fontWeight = FontWeight.Bold, color = Color(0xFFFFFFFF))
            Text("A full-screen SwiftUI and Compose app rendered from one .crepus file.", fontSize = 14.0.sp, color = Color(0xFFE5E7EB))
        }
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Column(modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(Color(0xFF2563EB)).padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Build", fontSize = 12.0.sp, color = Color(0xFFDBEAFE))
                Text("Ready", fontSize = 24.0.sp, fontWeight = FontWeight.Bold, color = Color(0xFFFFFFFF))
            }
            Column(modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(Color(0xFF059669)).padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Runtime", fontSize = 12.0.sp, color = Color(0xFFD1FAE5))
                Text("Live", fontSize = 24.0.sp, fontWeight = FontWeight.Bold, color = Color(0xFFFFFFFF))
            }
        }
        Column(modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(Color(0xFF1F2937)).padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text("Today", fontSize = 14.0.sp, fontWeight = FontWeight.SemiBold, color = Color(0xFFFFFFFF))
            Text("Edit views/main.crepus and run crepus mobile dev for hot reload.", fontSize = 14.0.sp, color = Color(0xFFE5E7EB))
            Text("Ship one View IR tree to iOS and Android.", fontSize = 14.0.sp, color = Color(0xFFE5E7EB))
        }
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Button(onClick = { CrepusActions.perform("sync") }, modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(Color(0xFFFFFFFF)).padding(16.dp)) {
                Text("Sync")
            }
            Button(onClick = { CrepusActions.perform("preview") }, modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(Color(0xFF334155)).padding(16.dp)) {
                Text("Preview")
            }
        }
    }
}
