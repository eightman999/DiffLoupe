import os
import difflib
import chardet
from PIL import Image
from PySide6.QtWidgets import(
    QMainWindow, QPushButton, QFileDialog, QTreeWidget, QVBoxLayout, QWidget,
    QLabel, QHBoxLayout, QTreeWidgetItem, QApplication, QTextEdit, QSplitter,
    QStackedWidget, QPlainTextEdit, QComboBox, QProgressBar, QCheckBox,
    QSlider, QButtonGroup, QRadioButton, QScrollArea, QSizePolicy
)
from PySide6.QtGui import QBrush, QColor, QFont, QPainter, QImage, QPixmap, QTextCursor, QTextFormat, QKeySequence, QShortcut
from PySide6.QtCore import QTimer
from PySide6.QtCore import Qt, QSize, QThread, Signal, QRect
from comparator import compare_all

# --- Background Worker Threads ---
class FolderComparisonWorker(QThread):
    comparison_finished = Signal(list)
    progress_updated = Signal(str)
    
    def __init__(self, folder_a, folder_b, show_hidden=False):
        super().__init__()
        self.folder_a = folder_a
        self.folder_b = folder_b
        self.show_hidden = show_hidden
    
    def run(self):
        self.progress_updated.emit("比較中...")
        try:
            results = compare_all([self.folder_a, self.folder_b], self.show_hidden)
            self.comparison_finished.emit(results)
        except Exception as e:
            self.progress_updated.emit(f"比較エラー: {str(e)}")

class FileContentWorker(QThread):
    content_loaded = Signal(str, list, list)  # encoding, lines_a, lines_b
    error_occurred = Signal(str)
    
    def __init__(self, file_path_a, file_path_b, encoding_mode):
        super().__init__()
        self.file_path_a = file_path_a
        self.file_path_b = file_path_b
        self.encoding_mode = encoding_mode
    
    def _read_lines(self, file_path, encoding_mode):
        if not file_path or not os.path.exists(file_path):
            return []
        try:
            if encoding_mode == "Auto-detect":
                with open(file_path, 'rb') as f:
                    raw_data = f.read()
                    result = chardet.detect(raw_data)
                    detected_encoding = result['encoding'] or 'utf-8'
                return raw_data.decode(detected_encoding, errors='ignore').splitlines(True)
            else:
                # エンコーディング名の正規化
                encoding_aliases = {
                    'KS_C_5601-1987': 'euc-kr',
                    'ASCII': 'ascii'
                }
                actual_encoding = encoding_aliases.get(encoding_mode, encoding_mode)
                
                with open(file_path, 'r', encoding=actual_encoding, errors='ignore') as f:
                    return f.readlines()
        except (IOError, UnicodeDecodeError, LookupError) as e:
            return [f"Error: Could not read file with encoding '{encoding_mode}': {str(e)}"]
    
    def run(self):
        try:
            lines_a = self._read_lines(self.file_path_a, self.encoding_mode)
            lines_b = self._read_lines(self.file_path_b, self.encoding_mode)
            self.content_loaded.emit(self.encoding_mode, lines_a, lines_b)
        except Exception as e:
            self.error_occurred.emit(str(e))

class ImageLoadWorker(QThread):
    image_loaded = Signal(object, str)  # QImage, side
    error_occurred = Signal(str, str)  # error_message, side
    
    def __init__(self, file_path, side):
        super().__init__()
        self.file_path = file_path
        self.side = side
    
    def run(self):
        if not self.file_path or not os.path.isfile(self.file_path):
            self.error_occurred.emit("ファイルなし", self.side)
            return
            
        try:
            image = QImage()
            if self.file_path.lower().endswith(('.dds', '.tga')):
                from PIL import Image
                pil_image = Image.open(self.file_path)
                pil_image = pil_image.convert("RGBA")
                image = QImage(pil_image.tobytes(), pil_image.size[0], pil_image.size[1], QImage.Format_RGBA8888)
            else:
                image = QImage(self.file_path)
            
            if image.isNull():
                self.error_occurred.emit("画像プレビュー非対応", self.side)
            else:
                self.image_loaded.emit(image, self.side)
        except Exception as e:
            self.error_occurred.emit(f"画像読み込みエラー: {str(e)}", self.side)

class HexLoadWorker(QThread):
    hex_loaded = Signal(str)
    error_occurred = Signal(str)
    
    def __init__(self, file_path):
        super().__init__()
        self.file_path = file_path
    
    def run(self):
        if not self.file_path or not os.path.isfile(self.file_path):
            self.error_occurred.emit("ファイルなし")
            return
            
        try:
            with open(self.file_path, 'rb') as f:
                content = f.read(1024 * 1024)  # Limit to 1MB
                hex_representation = ' '.join([f'{b:02x}' for b in content])
                self.hex_loaded.emit(hex_representation)
        except Exception as e:
            self.error_occurred.emit(f"ファイル読み込みエラー: {str(e)}")

# --- Custom Line Number Widget ---
class LineNumberArea(QWidget):
    def __init__(self, editor):
        super().__init__(editor)
        self.editor = editor

    def sizeHint(self):
        return QSize(self.editor.lineNumberAreaWidth(), 0)

    def paintEvent(self, event):
        self.editor.lineNumberAreaPaintEvent(event)

class CodeEditor(QPlainTextEdit):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.lineNumberArea = LineNumberArea(self)
        self.blockCountChanged.connect(self.updateLineNumberAreaWidth)
        self.updateRequest.connect(self.updateLineNumberArea)
        self.updateLineNumberAreaWidth(0)

    def lineNumberAreaWidth(self):
        digits = 1
        max_num = max(1, self.blockCount())
        while max_num >= 10:
            max_num //= 10
            digits += 1
        space = 3 + self.fontMetrics().horizontalAdvance('9') * digits
        return space

    def updateLineNumberAreaWidth(self, _):
        self.setViewportMargins(self.lineNumberAreaWidth(), 0, 0, 0)

    def updateLineNumberArea(self, rect, dy):
        if dy:
            self.lineNumberArea.scroll(0, dy)
        else:
            self.lineNumberArea.update(0, rect.y(), self.lineNumberArea.width(), rect.height())
        if rect.contains(self.viewport().rect()):
            self.updateLineNumberAreaWidth(0)

    def resizeEvent(self, event):
        super().resizeEvent(event)
        cr = self.contentsRect()
        self.lineNumberArea.setGeometry(cr.left(), cr.top(), self.lineNumberAreaWidth(), cr.height())

    def lineNumberAreaPaintEvent(self, event):
        painter = QPainter(self.lineNumberArea)
        painter.fillRect(event.rect(), QColor("#f0f0f0"))
        block = self.firstVisibleBlock()
        blockNumber = block.blockNumber()
        top = self.blockBoundingGeometry(block).translated(self.contentOffset()).top()
        bottom = top + self.blockBoundingRect(block).height()

        while block.isValid() and top <= event.rect().bottom():
            if block.isVisible() and bottom >= event.rect().top():
                number = str(blockNumber + 1)
                painter.setPen(Qt.black)
                painter.drawText(0, int(top), self.lineNumberArea.width() - 3, self.fontMetrics().height(),
                                 Qt.AlignRight, number)
            block = block.next()
            top = bottom
            bottom = top + self.blockBoundingRect(block).height()
            blockNumber += 1

# --- Diff Viewer Widget ---
class DiffViewer(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.editor_a = CodeEditor()
        self.editor_b = CodeEditor()
        self.editor_a.setReadOnly(True)
        self.editor_b.setReadOnly(True)
        self.content_worker = None

        layout = QHBoxLayout()
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(0)
        layout.addWidget(self.editor_a)
        layout.addWidget(self.editor_b)
        self.setLayout(layout)

        # Sync scrolling
        self.editor_a.verticalScrollBar().valueChanged.connect(self.editor_b.verticalScrollBar().setValue)
        self.editor_b.verticalScrollBar().valueChanged.connect(self.editor_a.verticalScrollBar().setValue)

    def clear(self):
        # コンテンツワーカーを停止
        if self.content_worker and self.content_worker.isRunning():
            self.content_worker.quit()
            self.content_worker.wait(1000)
            if self.content_worker.isRunning():
                self.content_worker.terminate()
                self.content_worker.wait()
        
        self.editor_a.clear()
        self.editor_b.clear()


    def set_files(self, file_path_a, file_path_b, encoding_mode):
        self.clear()
        self.editor_a.setPlainText("読み込み中...")
        self.editor_b.setPlainText("読み込み中...")
        
        if self.content_worker and self.content_worker.isRunning():
            self.content_worker.quit()
            self.content_worker.wait()
        
        self.content_worker = FileContentWorker(file_path_a, file_path_b, encoding_mode)
        self.content_worker.content_loaded.connect(self._on_content_loaded)
        self.content_worker.error_occurred.connect(self._on_content_error)
        self.content_worker.start()
    
    def _on_content_loaded(self, encoding_mode, lines_a, lines_b):
        self.clear()
        self.show_diff(lines_a, lines_b)
    
    def _on_content_error(self, error_message):
        self.clear()
        self.editor_a.setPlainText(f"エラー: {error_message}")
        self.editor_b.setPlainText(f"エラー: {error_message}")

    CONTEXT_LINES = 3  # 変更箇所の前後に表示する行数
    SKIP_THRESHOLD = 10 # この行数以上連続で一致する場合に省略する

    def show_diff(self, fromlines, tolines):
        diff = difflib.SequenceMatcher(None, fromlines, tolines)
        
        added_color = QColor("#d4edda")
        removed_color = QColor("#f8d7da")
        skipped_color = QColor("#e0e0e0") # 省略行の背景色

        for opcode, i1, i2, j1, j2 in diff.get_opcodes():
            if opcode == 'equal':
                num_equal_lines = i2 - i1
                if num_equal_lines > self.SKIP_THRESHOLD:
                    # 省略前のコンテキスト行を表示
                    for i in range(min(self.CONTEXT_LINES, num_equal_lines)):
                        self.append_text(self.editor_a, fromlines[i1 + i].rstrip('\r\n'))
                        self.append_text(self.editor_b, tolines[j1 + i].rstrip('\r\n'))
                    
                    # 省略メッセージを表示
                    skipped_message = f"<----------------- {i1 + self.CONTEXT_LINES + 1}行目から{i2 - self.CONTEXT_LINES}行目まで省略 ({num_equal_lines - 2 * self.CONTEXT_LINES}行) ----------------->"
                    self.append_text(self.editor_a, skipped_message, skipped_color)
                    self.append_text(self.editor_b, skipped_message, skipped_color)

                    # 省略後のコンテキスト行を表示
                    for i in range(num_equal_lines - self.CONTEXT_LINES, num_equal_lines):
                        self.append_text(self.editor_a, fromlines[i1 + i].rstrip('\r\n'))
                        self.append_text(self.editor_b, tolines[j1 + i].rstrip('\r\n'))
                else:
                    # 省略しない場合は全て表示
                    for line in fromlines[i1:i2]:
                        self.append_text(self.editor_a, line.rstrip('\r\n'))
                        self.append_text(self.editor_b, line.rstrip('\r\n'))
            else:
                len_a = i2 - i1
                len_b = j2 - j1
                max_len = max(len_a, len_b)

                for i in range(max_len):
                    if i < len_a:
                        self.append_text(self.editor_a, fromlines[i1 + i].rstrip('\r\n'), removed_color)
                    else:
                        self.append_text(self.editor_a, "", removed_color)

                    if i < len_b:
                        self.append_text(self.editor_b, tolines[j1 + i].rstrip('\r\n'), added_color)
                    else:
                        self.append_text(self.editor_b, "", added_color)

    def append_text(self, editor, text, color=None):
        cursor = editor.textCursor()
        cursor.movePosition(QTextCursor.End)
        
        if color:
            block_format = cursor.blockFormat()
            block_format.setBackground(QBrush(color))
            cursor.insertBlock(block_format)
            cursor.insertText(text)
        else:
            editor.appendPlainText(text)

# --- Enhanced Image Viewer Widget ---
class ImageComparisonViewer(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.image_a = None
        self.image_b = None
        self.file_path_a = None
        self.file_path_b = None
        self.image_workers = {}  # ワーカースレッド管理
        
        # 比較モード
        self.MODE_SIDE_BY_SIDE = 0
        self.MODE_TOGGLE = 1
        self.MODE_SLICE = 2
        self.current_mode = self.MODE_SIDE_BY_SIDE
        
        # スケーリングモード
        self.SCALE_FIT_WINDOW = 0
        self.SCALE_ACTUAL_SIZE = 1
        self.SCALE_CUSTOM = 2
        self.scale_mode = self.SCALE_FIT_WINDOW
        self.scale_factor = 1.0
        
        # タイマー（入れ替えモード用）
        self.toggle_timer = QTimer()
        self.toggle_timer.timeout.connect(self._toggle_images)
        self.toggle_state = False
        
        self.init_ui()
    
    def init_ui(self):
        layout = QVBoxLayout()
        
        # 比較モード選択
        mode_layout = QHBoxLayout()
        self.mode_group = QButtonGroup()
        
        self.side_by_side_radio = QRadioButton("左右比較")
        self.toggle_radio = QRadioButton("入れ替え")
        self.slice_radio = QRadioButton("スライス")
        
        self.side_by_side_radio.setChecked(True)
        
        self.mode_group.addButton(self.side_by_side_radio, self.MODE_SIDE_BY_SIDE)
        self.mode_group.addButton(self.toggle_radio, self.MODE_TOGGLE)
        self.mode_group.addButton(self.slice_radio, self.MODE_SLICE)
        
        mode_layout.addWidget(self.side_by_side_radio)
        mode_layout.addWidget(self.toggle_radio)
        mode_layout.addWidget(self.slice_radio)
        mode_layout.addStretch()
        
        # スケーリングモード選択
        self.fit_window_radio = QRadioButton("ウィンドウに合わせる")
        self.actual_size_radio = QRadioButton("実際のサイズ")
        self.fit_window_radio.setChecked(True)
        
        self.scale_group = QButtonGroup()
        self.scale_group.addButton(self.fit_window_radio, self.SCALE_FIT_WINDOW)
        self.scale_group.addButton(self.actual_size_radio, self.SCALE_ACTUAL_SIZE)
        
        mode_layout.addWidget(QLabel("|"))
        mode_layout.addWidget(self.fit_window_radio)
        mode_layout.addWidget(self.actual_size_radio)
        
        # ズームコントロール
        self.zoom_in_btn = QPushButton("拡大(+)")
        self.zoom_out_btn = QPushButton("縮小(-)")
        self.zoom_reset_btn = QPushButton("リセット")
        self.zoom_label = QLabel("100%")
        
        mode_layout.addWidget(QLabel("|"))
        mode_layout.addWidget(self.zoom_out_btn)
        mode_layout.addWidget(self.zoom_label)
        mode_layout.addWidget(self.zoom_in_btn)
        mode_layout.addWidget(self.zoom_reset_btn)
        
        # スライス用スライダー
        self.slice_slider = QSlider(Qt.Horizontal)
        self.slice_slider.setRange(0, 100)
        self.slice_slider.setValue(50)
        self.slice_slider.setVisible(False)
        self.slice_label = QLabel("スライス位置: 50%")
        self.slice_label.setVisible(False)
        
        # 入れ替え用コントロール
        self.toggle_speed_slider = QSlider(Qt.Horizontal)
        self.toggle_speed_slider.setRange(100, 3000)  # 0.1秒〜3秒
        self.toggle_speed_slider.setValue(1000)  # 1秒
        self.toggle_speed_slider.setVisible(False)
        self.toggle_speed_label = QLabel("切り替え間隔: 1.0秒")
        self.toggle_speed_label.setVisible(False)
        
        # 画像表示エリア
        self.scroll_area = QScrollArea()
        self.image_label = QLabel("画像を選択してください")
        self.image_label.setAlignment(Qt.AlignCenter)
        self.image_label.setStyleSheet("border: 1px solid gray;")
        self.image_label.setMinimumSize(400, 300)
        self.scroll_area.setWidget(self.image_label)
        self.scroll_area.setWidgetResizable(False)  # スケーリングで制御するため
        
        layout.addLayout(mode_layout)
        layout.addWidget(self.slice_label)
        layout.addWidget(self.slice_slider)
        layout.addWidget(self.toggle_speed_label)
        layout.addWidget(self.toggle_speed_slider)
        layout.addWidget(self.scroll_area)
        
        self.setLayout(layout)
        
        # シグナル接続
        self.mode_group.buttonClicked.connect(self._on_mode_changed)
        self.scale_group.buttonClicked.connect(self._on_scale_mode_changed)
        self.slice_slider.valueChanged.connect(self._on_slice_changed)
        self.toggle_speed_slider.valueChanged.connect(self._on_toggle_speed_changed)
        self.zoom_in_btn.clicked.connect(self._zoom_in)
        self.zoom_out_btn.clicked.connect(self._zoom_out)
        self.zoom_reset_btn.clicked.connect(self._zoom_reset)
    
    def _on_mode_changed(self, button):
        self.current_mode = self.mode_group.id(button)
        
        # UIの表示制御
        is_slice = (self.current_mode == self.MODE_SLICE)
        is_toggle = (self.current_mode == self.MODE_TOGGLE)
        
        self.slice_slider.setVisible(is_slice)
        self.slice_label.setVisible(is_slice)
        self.toggle_speed_slider.setVisible(is_toggle)
        self.toggle_speed_label.setVisible(is_toggle)
        
        # タイマー制御
        if is_toggle:
            self.toggle_timer.start(self.toggle_speed_slider.value())
        else:
            self.toggle_timer.stop()
        
        self._update_display()
    
    def _on_scale_mode_changed(self, button):
        self.scale_mode = self.scale_group.id(button)
        if self.scale_mode != self.SCALE_CUSTOM:
            self.scale_factor = 1.0
            self._update_zoom_label()
        self._update_display()
    
    def _zoom_in(self):
        self.scale_mode = self.SCALE_CUSTOM
        self.scale_factor = min(self.scale_factor * 1.25, 10.0)  # 最大10倍まで
        self._update_zoom_label()
        self._update_display()
    
    def _zoom_out(self):
        self.scale_mode = self.SCALE_CUSTOM
        self.scale_factor = max(self.scale_factor / 1.25, 0.1)  # 最小0.1倍まで
        self._update_zoom_label()
        self._update_display()
    
    def _zoom_reset(self):
        self.scale_factor = 1.0
        self.scale_mode = self.SCALE_FIT_WINDOW
        self.fit_window_radio.setChecked(True)
        self._update_zoom_label()
        self._update_display()
    
    def _update_zoom_label(self):
        zoom_percent = int(self.scale_factor * 100)
        self.zoom_label.setText(f"{zoom_percent}%")
    
    def _on_slice_changed(self, value):
        self.slice_label.setText(f"スライス位置: {value}%")
        if self.current_mode == self.MODE_SLICE:
            self._update_display()
    
    def _on_toggle_speed_changed(self, value):
        speed_sec = value / 1000.0
        self.toggle_speed_label.setText(f"切り替え間隔: {speed_sec:.1f}秒")
        if self.toggle_timer.isActive():
            self.toggle_timer.setInterval(value)
    
    def _toggle_images(self):
        self.toggle_state = not self.toggle_state
        self._update_display()
    
    def set_images(self, file_path_a, file_path_b):
        self.file_path_a = file_path_a
        self.file_path_b = file_path_b
        self.image_a = None
        self.image_b = None
        
        # 画像を非同期で読み込み
        if file_path_a:
            self._load_image_async(file_path_a, 'A')
        if file_path_b:
            self._load_image_async(file_path_b, 'B')
    
    def _load_image_async(self, file_path, side):
        # 既存のワーカーを停止
        worker_key = f"{side}_{file_path}"
        if worker_key in self.image_workers and self.image_workers[worker_key].isRunning():
            self.image_workers[worker_key].quit()
            self.image_workers[worker_key].wait(1000)
        
        # 新しいワーカーを作成
        worker = ImageLoadWorker(file_path, side)
        worker.image_loaded.connect(self._on_image_loaded)
        worker.error_occurred.connect(self._on_image_load_error)
        worker.finished.connect(lambda: self._remove_worker(worker_key))
        
        self.image_workers[worker_key] = worker
        worker.start()
    
    def _remove_worker(self, worker_key):
        """終了したワーカーを削除"""
        if worker_key in self.image_workers:
            del self.image_workers[worker_key]
    
    def _on_image_loaded(self, image, side):
        if side == 'A':
            self.image_a = image
        else:
            self.image_b = image
        
        self._update_display()
    
    def _on_image_load_error(self, error_msg, side):
        self.image_label.setText(f"画像読み込みエラー ({side}): {error_msg}")
    
    def _update_display(self):
        if not self.image_a and not self.image_b:
            self.image_label.setText("画像を選択してください")
            return
        
        if self.current_mode == self.MODE_SIDE_BY_SIDE:
            self._display_side_by_side()
        elif self.current_mode == self.MODE_TOGGLE:
            self._display_toggle()
        elif self.current_mode == self.MODE_SLICE:
            self._display_slice()
    
    def _display_side_by_side(self):
        if not self.image_a and not self.image_b:
            return
        
        # 両方の画像のサイズを取得
        size_a = self.image_a.size() if self.image_a else QSize(0, 0)
        size_b = self.image_b.size() if self.image_b else QSize(0, 0)
        
        # 表示サイズを計算
        max_width = max(size_a.width(), size_b.width())
        max_height = max(size_a.height(), size_b.height())
        
        # 横並び用の画像を作成
        combined_width = max_width * 2 + 10  # 間に余白
        combined_image = QImage(combined_width, max_height, QImage.Format_ARGB32)
        combined_image.fill(Qt.white)
        
        painter = QPainter(combined_image)
        try:
            # 画像Aを描画
            if self.image_a:
                painter.drawImage(0, 0, self.image_a)
            else:
                painter.fillRect(0, 0, max_width, max_height, Qt.lightGray)
                painter.drawText(0, 0, max_width, max_height, Qt.AlignCenter, "画像A\n(なし)")
            
            # 画像Bを描画
            if self.image_b:
                painter.drawImage(max_width + 10, 0, self.image_b)
            else:
                painter.fillRect(max_width + 10, 0, max_width, max_height, Qt.lightGray)
                painter.drawText(max_width + 10, 0, max_width, max_height, Qt.AlignCenter, "画像B\n(なし)")
        finally:
            painter.end()
        
        pixmap = QPixmap.fromImage(combined_image)
        scaled_pixmap = self._apply_scaling(pixmap)
        self.image_label.setPixmap(scaled_pixmap)
        self.image_label.resize(scaled_pixmap.size())
    
    def _display_toggle(self):
        if self.toggle_state:
            image_to_show = self.image_b if self.image_b else None
            label_text = "画像B" if self.image_b else "画像B (なし)"
        else:
            image_to_show = self.image_a if self.image_a else None
            label_text = "画像A" if self.image_a else "画像A (なし)"
        
        if image_to_show:
            pixmap = QPixmap.fromImage(image_to_show)
            scaled_pixmap = self._apply_scaling(pixmap)
            self.image_label.setPixmap(scaled_pixmap)
            self.image_label.resize(scaled_pixmap.size())
        else:
            self.image_label.setText(label_text)
    
    def _display_slice(self):
        if not self.image_a or not self.image_b:
            self.image_label.setText("スライス比較には両方の画像が必要です")
            return
        
        try:
            self._perform_slice_display()
        except Exception as e:
            self.image_label.setText(f"スライス表示エラー: {str(e)}")
    
    def _perform_slice_display(self):
        # 両画像のサイズを統一
        size_a = self.image_a.size()
        size_b = self.image_b.size()
        
        if size_a.width() <= 0 or size_a.height() <= 0 or size_b.width() <= 0 or size_b.height() <= 0:
            self.image_label.setText("無効な画像サイズです")
            return
        
        target_width = max(size_a.width(), size_b.width())
        target_height = max(size_a.height(), size_b.height())
        
        if target_width <= 0 or target_height <= 0:
            self.image_label.setText("無効なターゲットサイズです")
            return
        
        # 画像をリサイズ
        scaled_a = self.image_a.scaled(target_width, target_height, Qt.KeepAspectRatio, Qt.SmoothTransformation)
        scaled_b = self.image_b.scaled(target_width, target_height, Qt.KeepAspectRatio, Qt.SmoothTransformation)
        
        # スライス位置を計算
        slice_pos = int(target_width * self.slice_slider.value() / 100)
        slice_pos = max(0, min(slice_pos, target_width))  # 範囲制限
        
        # 合成画像を作成
        combined_image = QImage(target_width, target_height, QImage.Format_ARGB32)
        combined_image.fill(Qt.white)
        
        painter = QPainter(combined_image)
        try:
            # 左側に画像A
            if slice_pos > 0:
                left_rect = QRect(0, 0, slice_pos, target_height)
                painter.drawImage(left_rect, scaled_a, left_rect)
            
            # 右側に画像B
            if slice_pos < target_width:
                right_rect = QRect(slice_pos, 0, target_width - slice_pos, target_height)
                painter.drawImage(right_rect, scaled_b, right_rect)
            
            # 分割線を描画
            painter.setPen(QColor(255, 0, 0, 200))
            painter.drawLine(slice_pos, 0, slice_pos, target_height)
        finally:
            painter.end()
        
        pixmap = QPixmap.fromImage(combined_image)
        scaled_pixmap = self._apply_scaling(pixmap)
        self.image_label.setPixmap(scaled_pixmap)
        self.image_label.resize(scaled_pixmap.size())
    
    def clear(self):
        self.cleanup_threads()
        self.image_a = None
        self.image_b = None
        self.file_path_a = None
        self.file_path_b = None
        self.toggle_timer.stop()
        self.image_label.clear()
        self.image_label.setText("画像を選択してください")
    
    def cleanup_threads(self):
        """全てのワーカースレッドを終了"""
        for worker in self.image_workers.values():
            if worker and worker.isRunning():
                worker.quit()
                worker.wait(1000)
                if worker.isRunning():
                    worker.terminate()
                    worker.wait()
        self.image_workers.clear()
        self.toggle_timer.stop()
    
    def _apply_scaling(self, pixmap):
        """スケーリングモードに応じてピクセルマップをスケール"""
        if self.scale_mode == self.SCALE_ACTUAL_SIZE:
            # 実际のサイズで表示
            return pixmap
        elif self.scale_mode == self.SCALE_FIT_WINDOW:
            # ウィンドウに合わせてスケール
            available_size = self.scroll_area.viewport().size()
            # マージンを考慮
            available_size.setWidth(available_size.width() - 20)
            available_size.setHeight(available_size.height() - 20)
            
            if available_size.width() > 0 and available_size.height() > 0:
                return pixmap.scaled(available_size, Qt.KeepAspectRatio, Qt.SmoothTransformation)
            else:
                return pixmap
        else:
            # カスタムスケール
            scaled_size = pixmap.size() * self.scale_factor
            return pixmap.scaled(scaled_size, Qt.KeepAspectRatio, Qt.SmoothTransformation)
    
    def resizeEvent(self, event):
        """ウィンドウサイズ変更時に表示を更新"""
        super().resizeEvent(event)
        if self.scale_mode == self.SCALE_FIT_WINDOW:
            # ウィンドウに合わせるモードの場合は再描画
            self._update_display()
    
    def wheelEvent(self, event):
        """マウスホイールでズーム"""
        if event.modifiers() == Qt.ControlModifier:
            # Ctrl+ホイールでズーム
            angle_delta = event.angleDelta().y()
            if angle_delta > 0:
                self._zoom_in()
            else:
                self._zoom_out()
            event.accept()
        else:
            super().wheelEvent(event)

# --- Main Window ---
class MainWindow(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("ディフルーペ - フォルダ差分表示")
        self.setMinimumSize(1400, 900)
        self.folder_a = None
        self.folder_b = None
        self.folder_a_label = QLabel("フォルダA: (未選択)")
        self.folder_b_label = QLabel("フォルダB: (未選択)")
        self.item_map = {}
        self.IMAGE_EXTENSIONS = ('.tga', '.png', '.jpg', '.jpeg', '.dds', '.bmp')
        self.comparison_worker = None
        self.hex_worker = None
        self.progress_bar = None
        self.comparison_results = []  # 比較結果を保存
        self.current_filter = 0  # 現在のフィルターモード
        self.current_sort = 0  # 現在のソートモード
        self.init_ui()

    def create_viewer_panel(self):
        stacked_widget = QStackedWidget()
        # 0: Diff Viewer
        diff_viewer = DiffViewer()
        stacked_widget.addWidget(diff_viewer)
        # 1: Enhanced Image Viewer
        image_viewer = ImageComparisonViewer()
        stacked_widget.addWidget(image_viewer)
        # 2: Hex Viewer
        hex_viewer = QPlainTextEdit()
        hex_viewer.setReadOnly(True)
        hex_viewer.setFont(QFont("Courier New", 10))
        stacked_widget.addWidget(hex_viewer)
        return stacked_widget

    def init_ui(self):
        self.select_a_button = QPushButton("フォルダAを選択")
        self.select_b_button = QPushButton("フォルダBを選択")
        self.compare_button = QPushButton("比較 (F5)")
        top_controls_layout = QHBoxLayout()
        top_controls_layout.addWidget(self.select_a_button)
        top_controls_layout.addWidget(self.folder_a_label)
        top_controls_layout.addStretch()
        top_controls_layout.addWidget(self.select_b_button)
        top_controls_layout.addWidget(self.folder_b_label)
        top_controls_layout.addStretch()
        top_controls_layout.addWidget(self.compare_button)

        # 第1行: 表示モードとエンコーディング
        settings_row1 = QHBoxLayout()
        settings_row1.setSpacing(8)  # 水平スペースを調整
        settings_row1.addWidget(QLabel("表示モード:"))
        
        self.text_mode_btn = QPushButton("テキスト")
        self.image_mode_btn = QPushButton("画像")
        self.hex_mode_btn = QPushButton("バイナリ")
        self.text_mode_btn.setCheckable(True)
        self.image_mode_btn.setCheckable(True)
        self.hex_mode_btn.setCheckable(True)
        self.text_mode_btn.setChecked(True)
        
        # サイズを小さく設定
        for btn in [self.text_mode_btn, self.image_mode_btn, self.hex_mode_btn]:
            btn.setMaximumWidth(60)
        
        settings_row1.addWidget(self.text_mode_btn)
        settings_row1.addWidget(self.image_mode_btn)
        settings_row1.addWidget(self.hex_mode_btn)
        
        settings_row1.addWidget(QLabel("|"))  # 区切り
        settings_row1.addWidget(QLabel("エンコーディング:"))
        
        self.encoding_combo = QComboBox()
        self.encoding_combo.addItems([
            'Auto-detect', 'UTF-8', 'UTF-16', 'UTF-32',
            'Shift_JIS', 'EUC-JP', 'CP932', 'ISO-2022-JP',
            'Windows-1252', 'ISO-8859-1', 'Big5', 'GB2312', 'GBK',
            'KS_C_5601-1987', 'ASCII'
        ])
        self.encoding_combo.setMaximumWidth(120)
        settings_row1.addWidget(self.encoding_combo)
        
        settings_row1.addWidget(QLabel("|"))  # 区切り
        self.show_hidden_checkbox = QCheckBox("隠しファイル")
        self.show_hidden_checkbox.setChecked(False)
        self.show_hidden_checkbox.setToolTip("ドットで始まるファイル・フォルダを表示します")
        settings_row1.addWidget(self.show_hidden_checkbox)
        
        settings_row1.addStretch()
        
        # 第2行: フィルターコントロール
        settings_row2 = QHBoxLayout()
        settings_row2.setSpacing(8)  # 水平スペースを調整
        settings_row2.setContentsMargins(0, 0, 0, 0)  # マージンを削除
        settings_row2.addWidget(QLabel("フィルター:"))
        
        self.filter_all_radio = QRadioButton("すべて")
        self.filter_missing_new_radio = QRadioButton("欠落・新規")
        self.filter_diff_radio = QRadioButton("差分のみ")
        
        # ラジオボタンの高さを制限
        for radio in [self.filter_all_radio, self.filter_missing_new_radio, self.filter_diff_radio]:
            radio.setMaximumHeight(25)
            radio.setSizePolicy(QSizePolicy.Policy.Preferred, QSizePolicy.Policy.Maximum)
        
        self.filter_all_radio.setChecked(True)
        self.filter_all_radio.setToolTip("すべてのファイルを表示 (Ctrl+1)")
        self.filter_missing_new_radio.setToolTip("欠落しているファイルや新規ファイルのみ表示 (Ctrl+2)")
        self.filter_diff_radio.setToolTip("内容に差分があるファイルのみ表示 (Ctrl+3)")
        
        self.filter_group = QButtonGroup()
        self.filter_group.addButton(self.filter_all_radio, 0)
        self.filter_group.addButton(self.filter_missing_new_radio, 1)
        self.filter_group.addButton(self.filter_diff_radio, 2)
        
        settings_row2.addWidget(self.filter_all_radio)
        settings_row2.addWidget(self.filter_missing_new_radio)
        settings_row2.addWidget(self.filter_diff_radio)
        
        settings_row2.addWidget(QLabel("|"))  # 区切り
        
        # コンパクトなリセットボタン
        self.filter_reset_btn = QPushButton("リセット")
        self.filter_reset_btn.setToolTip("フィルターをリセットしてすべて表示 (Ctrl+R)")
        self.filter_reset_btn.setMaximumWidth(60)
        self.filter_reset_btn.setMaximumHeight(25)  # 高さを制限
        settings_row2.addWidget(self.filter_reset_btn)
        
        settings_row2.addWidget(QLabel("|"))  # 区切り
        
        # ファイル数表示ラベル
        self.file_count_label = QLabel("ファイル数: 0")
        self.file_count_label.setMinimumWidth(100)
        self.file_count_label.setMaximumHeight(25)  # 高さを制限
        settings_row2.addWidget(self.file_count_label)
        
        settings_row2.addWidget(QLabel("|"))  # 区切り
        
        # ソートコントロール
        settings_row2.addWidget(QLabel("ソート:"))
        
        self.sort_combo = QComboBox()
        self.sort_combo.addItems([
            "名前 (昇順)",
            "名前 (降順)", 
            "更新日 (昇順)",
            "更新日 (降順)",
            "サイズ (昇順)",
            "サイズ (降順)"
        ])
        self.sort_combo.setMaximumWidth(120)
        self.sort_combo.setMaximumHeight(25)
        self.sort_combo.setToolTip("ファイルの並び順を選択")
        settings_row2.addWidget(self.sort_combo)
        
        settings_row2.addStretch()

        self.tree_a = QTreeWidget()
        self.tree_a.setHeaderLabels(["ファイル"])
        self.tree_b = QTreeWidget()
        self.tree_b.setHeaderLabels(["ファイル"])

        self.viewer_stack = self.create_viewer_panel()

        main_splitter = QSplitter(Qt.Horizontal)
        main_splitter.addWidget(self.tree_a)
        main_splitter.addWidget(self.viewer_stack)
        main_splitter.addWidget(self.tree_b)
        main_splitter.setSizes([250, 900, 250])

        # Progress bar (initially hidden)
        self.progress_bar = QProgressBar()
        self.progress_bar.setVisible(False)
        self.progress_bar.setRange(0, 0)  # Indeterminate progress
        
        main_layout = QVBoxLayout()
        main_layout.setSpacing(3)  # 要素間のスペースをさらに減らす
        main_layout.setContentsMargins(5, 5, 5, 5)  # ウィンドウのマージンを減らす
        main_layout.addLayout(top_controls_layout)
        main_layout.addLayout(settings_row1)
        main_layout.addLayout(settings_row2)
        main_layout.addWidget(self.progress_bar)
        main_layout.addWidget(main_splitter)
        container = QWidget()
        container.setLayout(main_layout)
        self.setCentralWidget(container)

        self.select_a_button.clicked.connect(self.select_folder_a)
        self.select_b_button.clicked.connect(self.select_folder_b)
        self.compare_button.clicked.connect(self.compare_folders)
        self.tree_a.currentItemChanged.connect(self.on_tree_item_selected)
        self.tree_b.currentItemChanged.connect(self.on_tree_item_selected)
        self.text_mode_btn.clicked.connect(lambda: self.set_view_mode(0))
        self.image_mode_btn.clicked.connect(lambda: self.set_view_mode(1))
        self.hex_mode_btn.clicked.connect(lambda: self.set_view_mode(2))
        self.encoding_combo.currentTextChanged.connect(self.on_encoding_changed)
        self.show_hidden_checkbox.stateChanged.connect(self.on_hidden_files_changed)
        self.filter_group.buttonClicked.connect(self.on_filter_changed)
        self.filter_reset_btn.clicked.connect(self.reset_filter)
        self.sort_combo.currentIndexChanged.connect(self.on_sort_changed)
        
        # キーボードショートカットを設定
        self._setup_shortcuts()
    
    def _setup_shortcuts(self):
        """キーボードショートカットを設定"""
        # Ctrl+1: すべて表示
        shortcut_all = QShortcut(QKeySequence("Ctrl+1"), self)
        shortcut_all.activated.connect(lambda: self.filter_all_radio.setChecked(True) or self.on_filter_changed(self.filter_all_radio))
        
        # Ctrl+2: 欠落・新規のみ
        shortcut_missing = QShortcut(QKeySequence("Ctrl+2"), self)
        shortcut_missing.activated.connect(lambda: self.filter_missing_new_radio.setChecked(True) or self.on_filter_changed(self.filter_missing_new_radio))
        
        # Ctrl+3: 差分のみ
        shortcut_diff = QShortcut(QKeySequence("Ctrl+3"), self)
        shortcut_diff.activated.connect(lambda: self.filter_diff_radio.setChecked(True) or self.on_filter_changed(self.filter_diff_radio))
        
        # Ctrl+R: フィルターリセット
        shortcut_reset = QShortcut(QKeySequence("Ctrl+R"), self)
        shortcut_reset.activated.connect(self.reset_filter)
        
        # F5: 比較実行
        shortcut_compare = QShortcut(QKeySequence("F5"), self)
        shortcut_compare.activated.connect(self.compare_folders)
        
        # Ctrl+S: ソートメニューにフォーカス
        shortcut_sort = QShortcut(QKeySequence("Ctrl+S"), self)
        shortcut_sort.activated.connect(self.sort_combo.setFocus)
    
    def closeEvent(self, event):
        """アプリケーション終了時のクリーンアップ処理"""
        self._cleanup_threads()
        event.accept()
    
    def _cleanup_threads(self):
        """全てのワーカースレッドを終了"""
        # フォルダ比較ワーカーを終了
        if self.comparison_worker and self.comparison_worker.isRunning():
            self.comparison_worker.quit()
            self.comparison_worker.wait(5000)  # 5秒タイムアウト
            if self.comparison_worker.isRunning():
                self.comparison_worker.terminate()
                self.comparison_worker.wait()
        
        
        # Hex読み込みワーカーを終了
        if self.hex_worker and self.hex_worker.isRunning():
            self.hex_worker.quit()
            self.hex_worker.wait(1000)
            if self.hex_worker.isRunning():
                self.hex_worker.terminate()
                self.hex_worker.wait()
        
        # DiffViewerのコンテンツワーカーを終了
        diff_viewer = self.viewer_stack.widget(0)
        if isinstance(diff_viewer, DiffViewer) and diff_viewer.content_worker:
            if diff_viewer.content_worker.isRunning():
                diff_viewer.content_worker.quit()
                diff_viewer.content_worker.wait(1000)
                if diff_viewer.content_worker.isRunning():
                    diff_viewer.content_worker.terminate()
                    diff_viewer.content_worker.wait()
        
        # ImageComparisonViewerのスレッドを終了
        image_viewer = self.viewer_stack.widget(1)
        if isinstance(image_viewer, ImageComparisonViewer):
            image_viewer.cleanup_threads()

    def on_encoding_changed(self, text):
        self.on_tree_item_selected(self.tree_a.currentItem(), None)
    
    def on_hidden_files_changed(self, state):
        # 隠しファイル表示設定が変更された場合、再比較を促す
        if self.folder_a and self.folder_b:
            self.statusBar().showMessage("隠しファイル設定が変更されました。再比較してください。")
    
    def on_filter_changed(self, button):
        """フィルター設定変更時の処理"""
        self.current_filter = self.filter_group.id(button)
        if self.comparison_results:
            # 比較結果がある場合はフィルターを適用してツリーを再構築
            self._apply_filter_and_rebuild_trees()
            self.statusBar().showMessage(f"フィルターを適用しました: {self._get_filter_name()}")
    
    def _get_filter_name(self):
        """現在のフィルター名を取得"""
        if self.current_filter == 0:
            return "すべて"
        elif self.current_filter == 1:
            return "欠落・新規のみ"
        elif self.current_filter == 2:
            return "差分のみ"
        return "不明"
    
    def _should_include_file(self, entry):
        """フィルター条件に合致するか判定"""
        status = entry["status"]
        files = entry["files"]
        
        if self.current_filter == 0:  # すべて
            return True
        elif self.current_filter == 1:  # 欠落・新規のみ
            # 片方のフォルダにしか存在しないファイル
            file_a_exists = files[0]['path'] is not None
            file_b_exists = files[1]['path'] is not None
            return file_a_exists != file_b_exists  # XOR: 片方のみ存在
        elif self.current_filter == 2:  # 差分のみ
            # 内容に差分があるファイル
            return status == "差分"
        
        return False
    
    def on_sort_changed(self, index):
        """ソート設定変更時の処理"""
        self.current_sort = index
        if self.comparison_results:
            self._apply_filter_and_rebuild_trees()
            sort_name = self.sort_combo.currentText()
            self.statusBar().showMessage(f"ソートを適用しました: {sort_name}")
    
    def _sort_results(self, results):
        """結果をソート"""
        if self.current_sort == 0:  # 名前 (昇順)
            return sorted(results, key=lambda x: x['rel_path'].lower())
        elif self.current_sort == 1:  # 名前 (降順)
            return sorted(results, key=lambda x: x['rel_path'].lower(), reverse=True)
        elif self.current_sort == 2:  # 更新日 (昇順)
            return sorted(results, key=lambda x: self._get_latest_mtime(x['files']))
        elif self.current_sort == 3:  # 更新日 (降順)
            return sorted(results, key=lambda x: self._get_latest_mtime(x['files']), reverse=True)
        elif self.current_sort == 4:  # サイズ (昇順)
            return sorted(results, key=lambda x: self._get_max_size(x['files']))
        elif self.current_sort == 5:  # サイズ (降順)
            return sorted(results, key=lambda x: self._get_max_size(x['files']), reverse=True)
        else:
            return results
    
    def _get_latest_mtime(self, files):
        """ファイルリストから最新の更新日時を取得"""
        max_mtime = 0
        for file_info in files:
            if file_info['path'] and file_info['mtime'] > max_mtime:
                max_mtime = file_info['mtime']
        return max_mtime
    
    def _get_max_size(self, files):
        """ファイルリストから最大サイズを取得"""
        max_size = 0
        for file_info in files:
            if file_info['path'] and file_info['size'] > max_size:
                max_size = file_info['size']
        return max_size
    
    def reset_filter(self):
        """フィルターとソートをリセット"""
        self.filter_all_radio.setChecked(True)
        self.current_filter = 0
        self.sort_combo.setCurrentIndex(0)
        self.current_sort = 0
        if self.comparison_results:
            self._apply_filter_and_rebuild_trees()
            self.statusBar().showMessage("フィルターとソートをリセットしました。")
    
    def _update_file_count_display(self, filtered_count, total_count):
        """ファイル数表示を更新"""
        if self.current_filter == 0:
            self.file_count_label.setText(f"ファイル数: {total_count}")
        else:
            self.file_count_label.setText(f"ファイル数: {filtered_count}/{total_count}")

    def set_view_mode(self, mode_index):
        self.text_mode_btn.setChecked(mode_index == 0)
        self.image_mode_btn.setChecked(mode_index == 1)
        self.hex_mode_btn.setChecked(mode_index == 2)
        self.viewer_stack.setCurrentIndex(mode_index)
        self.on_tree_item_selected(self.tree_a.currentItem(), None)

    def on_tree_item_selected(self, current_item, previous_item):
        if not current_item or current_item.childCount() > 0:
            for i in range(3):
                self.clear_viewer(self.viewer_stack.widget(i))
            return

        rel_path = current_item.data(1, Qt.UserRole)
        if not rel_path or rel_path not in self.item_map:
            return

        sender_tree = self.sender()
        if sender_tree == self.tree_a:
            other_tree = self.tree_b
            sender_tree_idx, other_tree_idx = 0, 1
        else:
            other_tree = self.tree_a
            sender_tree_idx, other_tree_idx = 1, 0

        self.tree_a.blockSignals(True)
        self.tree_b.blockSignals(True)
        if other_tree_idx in self.item_map[rel_path]:
            other_item = self.item_map[rel_path][other_tree_idx]
            if other_tree.currentItem() != other_item:
                other_tree.setCurrentItem(other_item)
                other_tree.scrollToItem(other_item)
        self.tree_a.blockSignals(False)
        self.tree_b.blockSignals(False)

        item_a = self.item_map[rel_path].get(0)
        item_b = self.item_map[rel_path].get(1)
        file_path_a = item_a.data(0, Qt.UserRole) if item_a else None
        file_path_b = item_b.data(0, Qt.UserRole) if item_b else None

        is_image = bool(file_path_a and file_path_a.lower().endswith(self.IMAGE_EXTENSIONS)) or \
                   bool(file_path_b and file_path_b.lower().endswith(self.IMAGE_EXTENSIONS))
        self.image_mode_btn.setEnabled(is_image)

        current_mode_is_image = self.viewer_stack.currentIndex() == 1
        if is_image and not current_mode_is_image:
            self.set_view_mode(1)
        elif not is_image and current_mode_is_image:
            self.set_view_mode(0)
        else:
            self.load_files_into_viewer(file_path_a, file_path_b)

    def load_files_into_viewer(self, file_path_a, file_path_b):
        for i in range(self.viewer_stack.count()):
            self.clear_viewer(self.viewer_stack.widget(i))

        mode = self.viewer_stack.currentIndex()
        if mode == 0: # Diff view
            encoding = self.encoding_combo.currentText()
            diff_viewer = self.viewer_stack.widget(0)
            diff_viewer.set_files(file_path_a, file_path_b, encoding)
        elif mode == 1: # Image view
            image_viewer = self.viewer_stack.widget(1)
            image_viewer.set_images(file_path_a, file_path_b)
        elif mode == 2: # Hex view
            self.load_hex_into_viewer(file_path_a, self.viewer_stack.widget(2))


    def load_hex_into_viewer(self, file_path, viewer):
        if file_path and os.path.isfile(file_path):
            viewer.setPlainText("読み込み中...")
            
            if self.hex_worker and self.hex_worker.isRunning():
                self.hex_worker.quit()
                self.hex_worker.wait()
            
            self.hex_worker = HexLoadWorker(file_path)
            self.hex_worker.hex_loaded.connect(viewer.setPlainText)
            self.hex_worker.error_occurred.connect(
                lambda msg: viewer.setPlainText(f"エラー: {msg}")
            )
            self.hex_worker.start()
        else:
            viewer.setPlainText("ファイルなし")

    def clear_viewer(self, viewer):
        if isinstance(viewer, DiffViewer) or isinstance(viewer, QPlainTextEdit):
            viewer.clear()
        elif isinstance(viewer, ImageComparisonViewer):
            viewer.clear()
        elif isinstance(viewer, QLabel):
            viewer.clear()
            viewer.setText("Preview")

    def select_folder_a(self):
        folder = QFileDialog.getExistingDirectory(self, "フォルダAを選択")
        if folder:
            self.folder_a = folder
            self.folder_a_label.setText(f"フォルダA: {os.path.basename(folder)}")

    def select_folder_b(self):
        folder = QFileDialog.getExistingDirectory(self, "フォルダBを選択")
        if folder:
            self.folder_b = folder
            self.folder_b_label.setText(f"フォルダB: {os.path.basename(folder)}")

    def compare_folders(self):
        if not self.folder_a or not self.folder_b:
            self.statusBar().showMessage("2つのフォルダを選択してください。")
            return
        
        self.compare_button.setEnabled(False)
        self.progress_bar.setVisible(True)
        
        if self.comparison_worker and self.comparison_worker.isRunning():
            self.comparison_worker.quit()
            self.comparison_worker.wait()
        
        show_hidden = self.show_hidden_checkbox.isChecked()
        self.comparison_worker = FolderComparisonWorker(self.folder_a, self.folder_b, show_hidden)
        self.comparison_worker.comparison_finished.connect(self._on_comparison_finished)
        self.comparison_worker.progress_updated.connect(self.statusBar().showMessage)
        self.comparison_worker.finished.connect(self._on_comparison_worker_finished)
        self.comparison_worker.start()
    
    def _on_comparison_finished(self, results):
        self.comparison_results = results  # 結果を保存
        self.item_map = {}
        self._apply_filter_and_rebuild_trees()
        self.statusBar().showMessage("比較完了")
    
    def _apply_filter_and_rebuild_trees(self):
        """フィルターを適用してツリーを再構築"""
        if not self.comparison_results:
            return
        
        # フィルターを適用した結果を作成
        filtered_results = []
        for entry in self.comparison_results:
            if self._should_include_file(entry):
                filtered_results.append(entry)
        
        # ソートを適用
        sorted_results = self._sort_results(filtered_results)
        
        # ツリーをクリアして再構築
        self.item_map = {}
        self.populate_tree(self.tree_a, sorted_results, 0, self.folder_a)
        self.populate_tree(self.tree_b, sorted_results, 1, self.folder_b)
        
        # フィルター結果の統計を表示
        total_files = len(self.comparison_results)
        filtered_files = len(filtered_results)
        
        # ファイル数表示を更新
        self._update_file_count_display(filtered_files, total_files)
        
        if self.current_filter != 0:  # すべて以外の場合
            filter_name = self._get_filter_name()
            self.statusBar().showMessage(
                f"フィルター: {filter_name} - {filtered_files}/{total_files}ファイルを表示中"
            )
        
        # フィルター結果が空の場合の処理
        if filtered_files == 0 and self.current_filter != 0:
            self.statusBar().showMessage(f"フィルター条件に一致するファイルがありません。")
        
        # ツリーを展開して結果を見やすくする
        self._expand_filtered_trees(filtered_files)
    
    def _expand_filtered_trees(self, filtered_count):
        """フィルター結果に応じてツリーを展開"""
        # 結果が少ない場合は自動展開
        if filtered_count <= 50:  # 50ファイル以下の場合
            self.tree_a.expandAll()
            self.tree_b.expandAll()
        else:
            # 結果が多い場合はルートレベルのみ展開
            for i in range(self.tree_a.topLevelItemCount()):
                self.tree_a.topLevelItem(i).setExpanded(True)
            for i in range(self.tree_b.topLevelItemCount()):
                self.tree_b.topLevelItem(i).setExpanded(True)
    
    def _on_comparison_worker_finished(self):
        self.compare_button.setEnabled(True)
        self.progress_bar.setVisible(False)

    def populate_tree(self, tree_widget, results, folder_index, base_path):
        tree_widget.clear()
        nodes = {}
        
        # フォルダを作成するために必要なパスを集める
        required_folders = set()
        for entry in results:
            rel_path = entry["rel_path"]
            path_parts = rel_path.split(os.sep)
            # ファイルの親フォルダをすべて追加
            for i in range(len(path_parts) - 1):
                folder_path = os.sep.join(path_parts[:i+1])
                required_folders.add(folder_path)
        
        # フォルダをソートして作成（親フォルダから順番に）
        sorted_folders = sorted(required_folders, key=lambda x: x.count(os.sep))
        for folder_path in sorted_folders:
            if folder_path not in nodes:
                self._create_folder_path(tree_widget, nodes, folder_path)
        
        # ファイルを追加
        for entry in results:
            rel_path = entry["rel_path"]
            status = entry["status"]
            full_path = entry["files"][folder_index]['path']
            
            path_parts = rel_path.split(os.sep)
            file_name = path_parts[-1]
            
            # 親フォルダを取得
            if len(path_parts) > 1:
                parent_path = os.sep.join(path_parts[:-1])
                parent_item = nodes.get(parent_path, tree_widget.invisibleRootItem())
            else:
                parent_item = tree_widget.invisibleRootItem()
            
            # ファイルアイテムを作成
            item = QTreeWidgetItem([file_name])
            parent_item.addChild(item)
            nodes[rel_path] = item
            
            # スタイルを適用
            self.style_tree_item(item, rel_path, full_path, status)
            if rel_path not in self.item_map:
                self.item_map[rel_path] = {}
            self.item_map[rel_path][folder_index] = item
    
    def _create_folder_path(self, tree_widget, nodes, folder_path):
        """フォルダパスを再帰的に作成"""
        if folder_path in nodes:
            return nodes[folder_path]
        
        path_parts = folder_path.split(os.sep)
        folder_name = path_parts[-1]
        
        # 親フォルダを取得または作成
        if len(path_parts) > 1:
            parent_path = os.sep.join(path_parts[:-1])
            if parent_path not in nodes:
                self._create_folder_path(tree_widget, nodes, parent_path)
            parent_item = nodes[parent_path]
        else:
            parent_item = tree_widget.invisibleRootItem()
        
        # フォルダアイテムを作成
        item = QTreeWidgetItem([folder_name])
        parent_item.addChild(item)
        
        # フォルダのスタイルを設定
        item.setForeground(0, QBrush(QColor("navy")))
        font = item.font(0)
        font.setBold(True)
        item.setFont(0, font)
        
        nodes[folder_path] = item
        return item

    def style_tree_item(self, item, rel_path, full_path, status):
        item.setData(0, Qt.UserRole, full_path)
        item.setData(1, Qt.UserRole, rel_path)
        color = QColor("#ffffff")
        if status == "一致":
            color = QColor("#c8facc")
        elif status == "一致 (メタデータ)":
            color = QColor("#e0f0e3") # Slightly different green
        elif status == "差異":
            color = QColor("#fdb6b6")
        if full_path is None:
            item.setText(0, f"{item.text(0)} (欠落)")
            color = QColor("#dddddd")
            item.setForeground(0, QBrush(QColor("gray")))
        item.setBackground(0, QBrush(color))