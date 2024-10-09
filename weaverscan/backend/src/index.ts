import express from "express";
import cors from "cors";
import axios from "axios";

const app = express();
const PORT = 4000; // Puerto para tu backend

app.use(cors());
app.use(express.json());

app.get("/api/blockchain", async (req, res) => {
  try {
    const response = await axios.get("http://127.0.0.1:9090"); // Conexión a la blockchain
    res.json(response.data);
  } catch (error) {
    res.status(500).json({ error: "Error al conectar con la blockchain" });
  }
});

app.post("/api/transaction", async (req, res) => {
  const { sender, recipient, amount } = req.body;
  try {
    const response = await axios.post("http://127.0.0.1:9090", null, {
      params: {
        sender,
        recipient,
        amount,
      },
    });
    res
      .status(201)
      .json({ message: "Transaction created", data: response.data });
  } catch (error) {
    res.status(500).json({ error: "Error al crear la transacción" });
  }
});

app.listen(PORT, () => {
  console.log(`Server is running on http://localhost:${PORT}`);
});
