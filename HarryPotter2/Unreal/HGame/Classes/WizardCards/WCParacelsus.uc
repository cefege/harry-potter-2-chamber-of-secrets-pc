//===============================================================================
//  #15 Paracelsus
//===============================================================================

class  WCParacelsus extends goldcards;

#EXEC TEXTURE IMPORT NAME=WizardCardParacelsusTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Paracelsussmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardParacelsusBigTexture FILE=TEXTURES\menu\Folio\Cards\Paracelsusbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

#EXEC TEXTURE IMPORT NAME=WizCardParacelsusBigTextureLayer0 FILE=TEXTURES\menu\Folio\Cards\15_Paracelsus_1.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardParacelsusBigTextureLayer1 FILE=TEXTURES\menu\Folio\Cards\15_Paracelsus_2.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardParacelsusBigTextureLayer2 FILE=TEXTURES\menu\Folio\Cards\15_Paracelsus_3.bmp GROUP="Icons" FLAGS=2 MIPS=OFF


function PostBeginPlay()
{
	WizardName = "Paracelsus";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=15
    skin(0)=Texture'HProps.skins.WizardCardParacelsusTex0'
	textureBig=Texture'WizCardParacelsusBigTexture'
	strDescriptionId="WizCard_0053"

	bIsLayered=true;
	textureLayers(0)=Texture'WizCardParacelsusBigTextureLayer0';
	textureLayers(1)=Texture'WizCardParacelsusBigTextureLayer1';
	textureLayers(2)=Texture'WizCardParacelsusBigTextureLayer2';


}
