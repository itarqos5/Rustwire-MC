// Original synthetic packet-root oracle; calls cached official release APIs.
import java.io.ByteArrayOutputStream;
import java.lang.reflect.*;
import java.util.*;
public final class LegacyRegistryRootOracle {
    static byte[] root(int id, boolean named) {
        ByteArrayOutputStream b=new ByteArrayOutputStream();b.write(id);
        if(id==0)return b.toByteArray();if(named)b.writeBytes(new byte[]{0,0});
        switch(id) {
            case 1:b.write(7);break;
            case 2:b.writeBytes(new byte[]{0,7});break;
            case 3:case 5:b.writeBytes(new byte[]{0,0,0,7});break;
            case 4:case 6:b.writeBytes(new byte[]{0,0,0,0,0,0,0,7});break;
            case 7:case 11:case 12:b.writeBytes(new byte[]{0,0,0,0});break;
            case 8:b.writeBytes(new byte[]{0,0});break;
            case 9:b.writeBytes(new byte[]{0,0,0,0,0});break;
            case 10:b.write(0);break;
            default:throw new AssertionError();
        }
        return b.toByteArray();
    }
    static byte[] body(int id, boolean named) {
        if(!named)return root(id,false);
        ByteArrayOutputStream b=new ByteArrayOutputStream();
        b.writeBytes(new byte[]{0,0,0,0,0,0,(byte)255,0});b.writeBytes(root(id,true));
        RegistryTagsOracle.string(b,"test:dimension");RegistryTagsOracle.string(b,"test:world");
        b.writeBytes(new byte[17]);return b.toByteArray();
    }
    public static void main(String[] args) throws Exception {
        Class<?> constants=Class.forName("net.minecraft.SharedConstants");
        try{constants.getMethod("tryDetectVersion").invoke(null);}catch(NoSuchMethodException e){constants.getMethod("a").invoke(null);}
        Class.forName("net.minecraft.server.DispenserRegistry").getMethod("a").invoke(null);
        RegistryTagsOracle.buffer=Class.forName("net.minecraft.network.PacketDataSerializer");RegistryTagsOracle.bytebuf=Class.forName("io.netty.buffer.ByteBuf");
        boolean named=args[0].equals("763");
        Class<?> packet=Class.forName(named?"net.minecraft.network.protocol.game.PacketPlayOutLogin":"net.minecraft.network.protocol.configuration.ClientboundRegistryDataPacket");
        for(int id=0;id<=12;id++) {
            boolean accepted;String message="";
            try{RegistryTagsOracle.decode(packet,body(id,named));accepted=true;}
            catch(InvocationTargetException e){accepted=false;message=e.getCause().getClass().getSimpleName()+":"+e.getCause().getMessage();}
            System.out.println("root["+id+"] accepted="+accepted+" "+message);
            if(accepted!=(id==10))throw new AssertionError("unexpected root result "+id);
        }
    }
}
